mod kubernetes;
pub mod models;
mod runpod;

use crate::config::{FactoryConf, CONFIG};
use crate::db::get_executor;
use crate::utils::ModelRef;
use kubernetes::KubernetesModelFactory;
use log::{error, info};
use models::DynModel;
use once_cell::sync::OnceCell;
use runpod::RunpodModelFactory;
use sqlx::query;
use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::process::exit;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

static MF: OnceCell<PersistentModelFactory> = OnceCell::new();

pub async fn init_factories() {
    let rmf: Box<dyn ModelFactory> = match CONFIG.factory {
        FactoryConf::Runpod(_) => Box::new(RunpodModelFactory::new()),
        FactoryConf::Kubernetes(_) => match KubernetesModelFactory::new().await {
            Ok(mf) => Box::new(mf),
            Err(err) => {
                error!("Error while connecting to kubernetes: {err}");
                exit(1)
            }
        },
    };
    let mf = PersistentModelFactory::new(rmf);
    match MF.set(mf) {
        Ok(()) => {}
        Err(_) => panic!("init_factories should only be called once"),
    };
    start_background_auto_clean(get_factory()).await;
}

pub fn get_factory() -> &'static PersistentModelFactory {
    MF.get()
        .expect("you tried to access the model factory before it was initialized")
}

#[async_trait::async_trait]
pub trait ModelFactory: Send + Sync {
    async fn create(&self, model: ModelRef) -> Result<DynModel, anyhow::Error>;
    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error>;
    async fn dump(&self, model: &DynModel) -> Result<Vec<u8>, anyhow::Error>;
    async fn restore(&self, dump: &[u8]) -> Result<DynModel, anyhow::Error>;
}

pub struct PersistentModelFactory {
    parent: Box<dyn ModelFactory>,
    model_locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    auto_clean_running: Mutex<bool>,
}

pub async fn start_background_auto_clean(pmf: &'static PersistentModelFactory) {
    if *pmf.auto_clean_running.lock().await {
        return;
    }
    {
        let mut locked = pmf.auto_clean_running.lock().await;
        *locked = true;
    }
    tokio::task::spawn(async {
        loop {
            match pmf.auto_clean().await {
                Ok(()) => {}
                Err(err) => {
                    error!("Error while auto cleaning: {err}");
                }
            }
            sleep(Duration::from_secs(60)).await;
        }
    });
}

impl PersistentModelFactory {
    pub fn new(parent: Box<dyn ModelFactory>) -> PersistentModelFactory {
        PersistentModelFactory {
            parent,
            model_locks: Mutex::new(HashMap::new()),
            auto_clean_running: Mutex::new(false),
        }
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
        let results = query!("SELECT data, last_request, unixepoch() AS now FROM instance")
            .fetch_all(get_executor())
            .await?;
        for item in results {
            let model = self.parent.restore(&item.data).await?;
            let timeout = CONFIG
                .models
                .get(model.name().await)
                .or_else(|| CONFIG.models.get("*"))
                .and_then(|model| model.timeout)
                .unwrap_or(CONFIG.model_timeout);
            if item.last_request + i64::from(timeout) > item.now {
                continue;
            }
            info!(
                "Cleaning instance {} for {}",
                model.id().await,
                model.name().await
            );
            self.destroy(model).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ModelFactory for PersistentModelFactory {
    async fn create(&self, model: ModelRef) -> Result<DynModel, anyhow::Error> {
        let model_ref = model.to_string();
        let lock = self.get_lock(&model_ref).await;
        let _lock_guard = lock.lock().await;
        let found = query!("SELECT data FROM instance WHERE model = ?", model_ref)
            .fetch_optional(get_executor())
            .await?;
        if let Some(obj) = found {
            let existing_model = self.parent.restore(&obj.data).await?;
            let model_id = existing_model.id().await;

            query!(
                "UPDATE instance SET last_request = unixepoch() WHERE id = ?",
                model_id
            )
            .fetch_optional(get_executor())
            .await?;

            Ok(existing_model)
        } else {
            let new_model = self.parent.create(model).await?;
            let model_id = new_model.id().await;
            info!("Created instance {model_id} for {model_ref}");
            let model_dump = self.parent.dump(&new_model).await?;

            query!("INSERT INTO instance (id, model, data, last_request) VALUES (?, ?, ?, unixepoch())", model_id, model_ref, model_dump).fetch_optional(get_executor()).await?;
            Ok(new_model)
        }
    }

    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error> {
        let lock = self.get_lock(model.name().await).await;
        let _lock_guard = lock.lock().await;
        let id = model.id().await.to_string();
        self.parent.destroy(model).await?;
        query!("DELETE FROM instance WHERE id = ?", id)
            .fetch_optional(get_executor())
            .await?;
        Ok(())
    }

    async fn dump(&self, model: &DynModel) -> Result<Vec<u8>, anyhow::Error> {
        self.parent.dump(model).await
    }

    async fn restore(&self, dump: &[u8]) -> Result<DynModel, anyhow::Error> {
        self.parent.restore(dump).await
    }
}

pub struct ModelNotFoundError {
    id: String,
}

impl Debug for ModelNotFoundError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl Display for ModelNotFoundError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "model with id {} not found", self.id)
    }
}

impl Error for ModelNotFoundError {}
