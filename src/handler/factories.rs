use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::time::Duration;
use log::{error, info};
use once_cell::sync::OnceCell;
use sqlx::query;
use tokio::sync::Mutex;
use tokio::time::sleep;
use crate::config::CONFIG;
use crate::db::sql::get_executor;
use crate::handler::models::DynModel;
use crate::handler::runpod::RunpodModelFactory;

static MF: OnceCell<PersistentModelFactory> = OnceCell::new();

pub async fn init_factories() {
    let rmf = RunpodModelFactory::new(CONFIG.runpod_token.clone());
    let mf = PersistentModelFactory::new(rmf);
    match MF.set(mf) {
        Ok(()) => {},
        Err(_) => panic!("init_factories should only be called once")
    };
    start_background_auto_clean(get_factory()).await;
}

pub fn get_factory() -> &'static PersistentModelFactory {
    MF.get().expect("you tried to access the model factory before it was initialized")
}


#[async_trait::async_trait]
pub trait ModelFactory: Send + Sync {
    async fn create(&self, model: String) -> Result<DynModel, anyhow::Error>;
    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error>;
    async fn dump(&self, model: &DynModel) -> Result<Vec<u8>, anyhow::Error>;
    async fn restore(&self, dump: &[u8]) -> Result<DynModel, anyhow::Error>;
}

pub struct PersistentModelFactory {
    parent: Box<dyn ModelFactory>,
    auto_clean_running: Mutex<bool>,
}


pub async fn start_background_auto_clean(pmf: &'static PersistentModelFactory) {
    if *pmf.auto_clean_running.lock().await {
        return
    }
    {
        let mut locked = pmf.auto_clean_running.lock().await;
        *locked = true;
    }
    tokio::task::spawn(async {
        loop {
            match pmf.auto_clean().await {
                Ok(()) => {},
                Err(err) => {
                    error!("Error while auto cleaning: {err}");
                }
            }
            sleep(Duration::from_secs(60)).await;
        }
    });
}


impl PersistentModelFactory {
    pub fn new(parent: impl ModelFactory + 'static) -> PersistentModelFactory {
        PersistentModelFactory{ parent: Box::new(parent), auto_clean_running: Mutex::new(false) }
    }

    pub async fn auto_clean(&self) -> Result<(), anyhow::Error> {
        let results = query!("SELECT data FROM instance WHERE last_request < unixepoch() - ?", CONFIG.model_timeout).fetch_all(get_executor()).await?;
        for item in results {
            let model = self.parent.restore(&item.data).await?;
            info!("Cleaning instance {} for {}", model.id().await, model.name().await);
            self.destroy(model).await?;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl ModelFactory for PersistentModelFactory {
    async fn create(&self, model: String) -> Result<DynModel, anyhow::Error> {
        let model_ref = &model;
        let found = query!("SELECT data FROM instance WHERE model = ?", model_ref).fetch_optional(get_executor()).await?;
        if let Some(obj) = found {
            let existing_model = self.parent.restore(&obj.data).await?;
            let model_id = existing_model.id().await;

            query!("UPDATE instance SET last_request = unixepoch() WHERE id = ?", model_id).fetch_optional(get_executor()).await?;

            Ok(existing_model)
        } else {
            let new_model = self.parent.create(model.clone()).await?;
            let model_id = new_model.id().await;
            info!("Creating instance {} for {}", model_id, model);
            let model_dump = self.parent.dump(&new_model).await?;

            query!("INSERT INTO instance (id, model, data, last_request) VALUES (?, ?, ?, unixepoch())", model_id, model, model_dump).fetch_optional(get_executor()).await?;
            Ok(new_model)
        }
    }

    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error> {
        let id = model.id().await;
        query!("DELETE FROM instance WHERE id = ?", id).fetch_optional(get_executor()).await?;
        self.parent.destroy(model).await
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

impl Error for ModelNotFoundError {

}
