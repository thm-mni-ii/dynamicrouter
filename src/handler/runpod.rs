use crate::handler::factories::ModelFactory;
use crate::handler::models::{DynModel, HttpModel};
use once_cell::sync::Lazy;
use reqwest::{Client, Url};
use serde_json::{json, Value};
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};

pub struct RunpodModelFactory {
    client: Client,
    token: String,
}

impl RunpodModelFactory {
    pub fn new(token: String) -> RunpodModelFactory {
        RunpodModelFactory{ client: Client::default(), token }
    }
}

static RUNPOD_GRAPHQL: Lazy<Url> = Lazy::new(|| "https://api.runpod.io/graphql".parse().unwrap());
const RUNPOD_CREATE_MUTATION: &str = "mutation Mutation($input: PodFindAndDeployOnDemandInput) {  podFindAndDeployOnDemand(input: $input) {    id    imageName    env    machineId    machine {      podHostId      __typename    }    __typename  }}";
const RUNPOD_DESTROY_MUTATION: &str = "mutation terminatePod($input: PodTerminateInput!) {podTerminate(input: $input)}";

#[async_trait::async_trait]
impl ModelFactory for RunpodModelFactory {

    async fn create(&self, model: String) -> Result<DynModel, anyhow::Error> {
        let res: Value = self.client.post(RUNPOD_GRAPHQL.clone())
            .bearer_auth(&self.token)
            .json(&json!({"query": RUNPOD_CREATE_MUTATION, "operationName": "Mutation", "variables": {"input":{
                "cloudType":"SECURE",
                "containerDiskInGb":400,
                "volumeInGb":0,
                "gpuCount":1,
                "gpuTypeId":"NVIDIA L40S",
                "templateId":"5pxt3e1avi",
                "ports":"80/http",
                "dockerArgs": format!("--model-id {} --quantize fp8", model)
            }}}))
            .send().await?.error_for_status()?.json().await?;
        let id = res.get("data").and_then(|c| c.get("podFindAndDeployOnDemand").and_then(|c| c.get("id")).and_then(|id| id.as_str())).ok_or_else(|| NoIdError{content: res.clone()})?;
        Ok(HttpModel::new_dyn(
            id.to_string(),
            model,
            format!("https://{id}-80.proxy.runpod.net/v1/chat/completions").parse()?,
             None,
        ))
    }

    async fn destroy(&self, model: DynModel) -> Result<(), anyhow::Error> {
        let id = model.id().await;
        self.client.post(RUNPOD_GRAPHQL.clone())
            .bearer_auth(&self.token)
            .json(&json!({"query": RUNPOD_DESTROY_MUTATION, "operationName": "terminatePod", "variables": {"input": {"podId": id}}}))
            .send().await?.error_for_status()?;
        Ok(())
    }

    async fn dump(&self, model: &DynModel) -> Result<Vec<u8>, anyhow::Error> {
        model.serialize().await
    }

    async fn restore(&self, dump: &[u8]) -> Result<DynModel, anyhow::Error> {
        HttpModel::deserialize(dump)
    }
}

struct NoIdError {
    content: Value,
}

impl Debug for NoIdError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to extract id from {:?}", self.content)
    }
}

impl Display for NoIdError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "failed to extract id from {:?}", self.content)
    }
}

impl Error for NoIdError {

}
