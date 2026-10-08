pub mod kubernetes;
pub mod models;
pub mod runpod;

use crate::utils::ModelRef;
use models::DynModel;
use std::sync::Arc;

#[async_trait::async_trait]
pub trait ModelFactory: Send + Sync {
    async fn create(&self, model: &ModelRef) -> Result<DynModel, anyhow::Error>;
    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error> {
        self.destroy_id(model.id().await).await
    }
    async fn destroy_id(&self, id: &str) -> Result<(), anyhow::Error>;
    async fn dump(&self, model: &DynModel) -> Result<Vec<u8>, anyhow::Error>;
    async fn restore(&self, dump: &[u8]) -> Result<DynModel, anyhow::Error>;
}

pub type DynModelFactory = Arc<dyn ModelFactory>;
