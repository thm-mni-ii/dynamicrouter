use crate::config::{get_model_config_by_name, FactoryConf, CONFIG};
use crate::db::instance::{create_instance, delete_instance, get_instances_for, Instance};
use crate::db::request::{
    count_pending_requests_for, create_request, latest_for_each_model, request_done,
};
use crate::fatal;
use crate::handlers::spawner::factories::kubernetes::KubernetesModelFactory;
use crate::handlers::spawner::factories::models::RetryingModel;
use crate::handlers::spawner::factories::runpod::RunpodModelFactory;
use crate::handlers::spawner::factories::{DynModelFactory, ModelFactory};
use crate::handlers::{DynHandler, Handler};
use crate::utils::response::ModelResponse;
use crate::utils::ModelRef;
use k8s_openapi::chrono::Utc;
use log::{error, info};
use rand::seq::IndexedRandom;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

mod backends;
mod factories;

pub struct SpawnerHandler {
    model_factory: DynModelFactory,
    model_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    pub auto_clean_running: Mutex<bool>,
}

impl SpawnerHandler {
    pub async fn new_dyn() -> DynHandler {
        let mf: Arc<dyn ModelFactory> = match CONFIG.factory {
            FactoryConf::Runpod(_) => Arc::new(RunpodModelFactory::new()),
            FactoryConf::Kubernetes(_) => match KubernetesModelFactory::new().await {
                Ok(mf) => Arc::new(mf),
                Err(err) => {
                    fatal!("Error while connecting to kubernetes: {err}");
                }
            },
        };
        let sh = Arc::new(SpawnerHandler {
            model_factory: mf,
            model_locks: Mutex::new(HashMap::new()),
            auto_clean_running: Mutex::new(false),
        });
        start_background_auto_clean(sh.clone()).await;
        sh
    }

    async fn get_lock(&self, model: &str) -> Arc<Mutex<()>> {
        let mut map_lock = self.model_locks.lock().await;
        let map_value = map_lock.contains_key(model);
        if !map_value {
            map_lock.insert(model.to_string(), Arc::new(Mutex::new(())));
        }
        map_lock.get(model).unwrap().clone()
    }

    pub async fn auto_clean(&self) -> Result<(), anyhow::Error> {
        let results = latest_for_each_model().await?;
        for item in results {
            let lock = self.get_lock(&item.model).await;
            let _lock_guard = lock.lock().await;
            let timeout = CONFIG
                .models
                .get(&item.model)
                .or_else(|| CONFIG.models.get("*"))
                .and_then(|model| model.timeout)
                .unwrap_or(CONFIG.model_timeout);
            if item.done.is_some() || item.enqueue + i64::from(timeout) > Utc::now().timestamp() {
                continue;
            }
            let instances = get_instances_for(&item.model).await?;
            for instance in instances {
                let model = self.model_factory.restore(&instance.data).await?;
                info!(
                    "Cleaning instance {} for {}",
                    model.id().await,
                    model.name().await
                );
                self.model_factory.destroy(model).await?;
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
        model: &ModelRef,
        request: &Value,
    ) -> Result<ModelResponse, anyhow::Error> {
        let model_name = model.to_string();
        let config = get_model_config_by_name(&model_name)?;
        let handler_config = config.handler.as_spawner();

        let lock = self.get_lock(&model_name).await;
        let _lock_guard = lock.lock().await;
        let instances = get_instances_for(&model_name).await?;
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
        let request_id = create_request(&model_name, dyn_model.id().await).await?;
        drop(_lock_guard);

        let model = RetryingModel::new_dyn(dyn_model);
        let result = model.generate(request).await;

        request_done(&request_id).await?;
        result
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
            match sh.auto_clean().await {
                Ok(()) => {}
                Err(err) => {
                    error!("Error while auto cleaning: {err}");
                }
            }
            sleep(Duration::from_secs(60)).await;
        }
    });
}
