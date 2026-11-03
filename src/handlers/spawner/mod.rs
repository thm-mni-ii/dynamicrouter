use crate::config::{CONFIG, FactoryType};
use crate::db::instance::{Instance, create_instance, delete_instance, get_instances_for};
use crate::db::model::get_model_config;
use crate::db::request::{
    count_pending_requests_for, create_request, latest_for_each_model, request_done, request_failed,
};
use crate::fatal;
use crate::handlers::spawner::factories::kubernetes::KubernetesModelFactory;
use crate::handlers::spawner::factories::models::{ModelError, RetryingModel};
use crate::handlers::spawner::factories::runpod::RunpodModelFactory;
use crate::handlers::spawner::factories::{DynModelFactory, ModelFactory};
use crate::handlers::{DynHandler, GenerationType, Handler};
use crate::utils::ModelRef;
use crate::utils::response::{ModelResponse, RequestGuard, wrap_response_with_logging};
use k8s_openapi::chrono::Utc;
use log::{error, info};
use rand::seq::IndexedRandom;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

pub mod backends;
pub mod factories;

pub struct SpawnerHandler {
    model_factory: DynModelFactory,
    model_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    pub auto_clean_running: Mutex<bool>,
}

impl SpawnerHandler {
    pub async fn new_arc() -> Arc<SpawnerHandler> {
        let mf: Arc<dyn ModelFactory> = match CONFIG.factory_type {
            Some(FactoryType::Kubernetes) => match KubernetesModelFactory::new().await {
                Ok(mf) => Arc::new(mf),
                Err(err) => {
                    fatal!("Error while connecting to kubernetes: {err}");
                }
            },
            _ => Arc::new(RunpodModelFactory::new()),
        };
        let sh = Arc::new(SpawnerHandler {
            model_factory: mf,
            model_locks: Mutex::new(HashMap::new()),
            auto_clean_running: Mutex::new(false),
        });
        start_background_auto_clean(sh.clone()).await;
        sh
    }

    pub fn with_factory(model_factory: DynModelFactory) -> Arc<SpawnerHandler> {
        Arc::new(SpawnerHandler {
            model_factory,
            model_locks: Mutex::new(HashMap::new()),
            auto_clean_running: Mutex::new(false),
        })
    }

    pub async fn new_dyn() -> DynHandler {
        Self::new_arc().await
    }

    async fn get_lock(&self, model: &str) -> Arc<Mutex<()>> {
        let mut map_lock = self.model_locks.lock().await;
        if !map_lock.contains_key(model) {
            map_lock.insert(model.to_string(), Arc::new(Mutex::new(())));
        }
        map_lock.get(model).unwrap().clone()
    }

    pub async fn terminate_instance(&self, id: &str) -> Result<(), anyhow::Error> {
        let instance = match crate::db::instance::get_instance_by_id(id).await? {
            Some(inst) => Some(inst),
            None => {
                if let Some(stripped) = id.strip_prefix("dynamicrouter-inference-") {
                    crate::db::instance::get_instance_by_id(stripped).await?
                } else {
                    None
                }
            }
        };

        if let Some(inst) = instance {
            let lock = self.get_lock(&inst.model).await;
            let _lock_guard = lock.lock().await;

            if !inst.data.is_empty() {
                if let Err(e) = self.model_factory.destroy_id(&inst.id).await {
                    error!("Error destroying instance {} in model factory: {e}", inst.id);
                }
            }
            delete_instance(&inst.id).await?;
            if inst.id != id {
                let _ = delete_instance(id).await;
            }
        } else {
            if let Err(e) = self.model_factory.destroy_id(id).await {
                error!("Error destroying instance {id} in model factory: {e}");
            }
            delete_instance(id).await?;
        }

        Ok(())
    }

    pub async fn auto_clean(&self) -> Result<(), anyhow::Error> {
        let _ = crate::db::request::cleanup_timed_out_requests(i64::from(CONFIG.model_timeout)).await;
        let results = latest_for_each_model().await?;
        for item in results {
            let lock = self.get_lock(&item.model).await;
            let _lock_guard = lock.lock().await;
            let model_conf = get_model_config(&item.model).await.unwrap_or(None);
            let timeout = model_conf
                .and_then(|m| m.timeout)
                .unwrap_or(CONFIG.model_timeout);

            if item.enqueue + i64::from(timeout) > Utc::now().timestamp() {
                continue;
            }
            let instances = get_instances_for(&item.model).await?;
            for instance in instances {
                info!(
                    "Cleaning instance {} for {}",
                    instance.id,
                    item.model
                );
                if let Err(e) = self.model_factory.destroy_id(&instance.id).await {
                    error!("Error destroying instance {} for {}: {e}", instance.id, item.model);
                }
                delete_instance(&instance.id).await?;
            }
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Handler for SpawnerHandler {
    async fn generate(
        &self,
        generation_type: GenerationType,
        model: &ModelRef,
        request: &Value,
        username: &str,
        user_log_request_body: Option<bool>,
        prompt_tokens: i64,
        token_hash: Option<&str>,
    ) -> Result<ModelResponse, anyhow::Error> {
        let model_name = model.to_string();
        let config = get_model_config(&model_name)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Model configuration for '{model_name}' not found"))?;
        let handler_config = config.handler.as_spawner();

        let lock = self.get_lock(&model_name).await;
        let _lock_guard = lock.lock().await;
        let instances: Vec<Instance> = get_instances_for(&model_name)
            .await?
            .into_iter()
            .filter(|inst| !inst.data.is_empty())
            .collect();
        let requests = count_pending_requests_for(&model_name).await?;
        let request_count = requests as u32;
        let mut instance_count = instances.len() as u32;

        let mut dyn_model = None;
        while instance_count < handler_config.min_scale
            || ((request_count > handler_config.queue_length_for_scale_up || instance_count == 0)
                && instance_count < handler_config.max_scale)
        {
            let new_dyn_model = self.model_factory.create(model).await?;
            create_instance(&Instance {
                id: new_dyn_model.id().await.to_string(),
                model: new_dyn_model.name().await.to_string(),
                data: self.model_factory.dump(&new_dyn_model).await?,
            })
            .await?;
            dyn_model = Some(new_dyn_model);
            instance_count += 1;
        }

        let dyn_model = if let Some(dyn_model) = dyn_model {
            dyn_model
        } else {
            let instance = instances
                .choose(&mut rand::rng())
                .ok_or_else(|| ModelNotScalable {
                    id: model_name.clone(),
                })?;
            self.model_factory.restore(&instance.data).await?
        };
        let request_body = if CONFIG.should_log_request_body_for_user(user_log_request_body) {
            Some(request)
        } else {
            None
        };
        let request_id =
            create_request(&model_name, dyn_model.id().await, username, request_body, Some(prompt_tokens), token_hash).await?;
        let mut guard = RequestGuard::new(request_id.clone());
        drop(_lock_guard);

        let model = RetryingModel::new_dyn(dyn_model);
        let result = model.generate(generation_type, request).await;

        match &result {
            Ok(_) => {
                request_done(&request_id).await?;
                guard.mark_completed();
            }
            Err(_) => {
                request_failed(&request_id).await?;
                guard.mark_completed();
            }
        }

        let result = match result {
            Ok(response) => Ok(wrap_response_with_logging(
                response,
                request_id,
                prompt_tokens,
                generation_type,
            )),
            Err(err) => Err(err),
        };

        if let Err(ModelError::NonRetryableError(e)) = &result {
            let model_id = model.id().await.to_string();
            error!(
                "non retryable error while processing request for {} ({}): {}",
                model_id,
                model.name().await,
                e
            );
            if let Err(e) = self.model_factory.destroy(model).await {
                error!("error while destroying non retryable model {model_id}: {e}");
            }
            delete_instance(&model_id).await?;
        }

        Ok(result?)
    }
}

#[derive(thiserror::Error, Debug)]
#[error("model {id} has no running instances and can't be scaled up")]
pub struct ModelNotScalable {
    id: String,
}

pub async fn start_background_auto_clean(sh: Arc<SpawnerHandler>) {
    if *sh.auto_clean_running.lock().await {
        return;
    }
    {
        let mut locked = sh.auto_clean_running.lock().await;
        *locked = true;
    }
    tokio::task::spawn(async move {
        loop {
            if let Err(err) = sh.auto_clean().await {
                error!("Error while auto cleaning: {err}");
            }
            sleep(Duration::from_mins(1)).await;
        }
    });
}
