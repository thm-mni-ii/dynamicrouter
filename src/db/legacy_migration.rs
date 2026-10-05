use crate::db::alias::create_alias;
use crate::db::backend::{create_backend, create_backend_model_config};
use crate::db::connection::get_executor;
use crate::db::factory::create_factory_gpu;
use crate::db::model::{
    ModelConfig, ModelHandlerConfig, OpenAiHandlerConfig, SpawnerHandlerConfig, create_model,
};
use crate::db::openai_api::create_openai_api;
use crate::db::user::{add_user_token, create_user};
use crate::utils::QuantType;
use log::{info, warn};
use secrecy::SecretBox;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use url::Url;

#[derive(Deserialize, Debug)]
struct LegacyConf {
    port: Option<u16>,
    sqlite_url: Option<String>,
    model_timeout: Option<u32>,
    model_retry_interval: Option<u32>,
    log_request_body: Option<bool>,
    huggingface: Option<LegacyHuggingfaceConf>,
    tls: Option<LegacyTlsConfig>,
    #[serde(default)]
    models: HashMap<String, LegacyModel>,
    #[serde(default)]
    users: HashMap<String, LegacyUser>,
    #[serde(default)]
    aliases: HashMap<String, LegacyAlias>,
    #[serde(default)]
    openai_apis: HashMap<String, LegacyOpenAiApiConf>,
    #[serde(default)]
    backends: HashMap<String, LegacyBackendConf>,
    factory: Option<LegacyFactoryConf>,
}

#[derive(Deserialize, Debug)]
struct LegacyHuggingfaceConf {
    token: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(untagged)]
enum LegacyTlsConfig {
    TlsPathConfig { cert_path: String, key_path: String },
    TlsInlineConfig { cert: String, key: String },
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct LegacyModel {
    name: Option<String>,
    timeout: Option<u32>,
    limit_to_users: Option<HashSet<String>>,
    hidden: Option<bool>,
    handler: Option<LegacyHandler>,
    parameter_count: Option<u64>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum LegacyHandler {
    #[serde(rename = "spawner")]
    Spawner(LegacySpawnerModelHandlerConfig),
    #[serde(rename = "openai")]
    OpenAi(LegacyOpenAiModelHandlerConfig),
}

#[derive(Deserialize, Debug, Default)]
struct LegacySpawnerModelHandlerConfig {
    backends: Option<HashSet<String>>,
    min_scale: Option<u32>,
    max_scale: Option<u32>,
    queue_length_for_scale_up: Option<u32>,
}

#[derive(Deserialize, Debug, Default)]
struct LegacyOpenAiModelHandlerConfig {
    api: Option<String>,
    url: Option<Url>,
    token: Option<String>,
    model: Option<String>,
    timeout: Option<u32>,
    headers: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
struct LegacyUser {
    token_hashes: HashSet<String>,
    limit_to_models: Option<HashSet<String>>,
    log_request_body: Option<bool>,
}

#[derive(Deserialize, Debug)]
struct LegacyAlias {
    to: String,
}

#[derive(Deserialize, Debug)]
struct LegacyOpenAiApiConf {
    url: Url,
    token: Option<String>,
    prefix: Option<String>,
    models: Option<HashSet<String>>,
    timeout: Option<u32>,
    headers: Option<HashMap<String, String>>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum LegacyBackendConf {
    #[serde(rename = "tgi")]
    Tgi(LegacyBackendGlobalConf),
    #[serde(rename = "vllm")]
    Vllm(LegacyBackendGlobalConf),
    #[serde(rename = "llama_cpp")]
    LlamaCpp(LegacyBackendGlobalConf),
    #[serde(rename = "sglang")]
    Sglang(LegacyBackendGlobalConf),
}

#[derive(Deserialize, Debug)]
struct LegacyBackendGlobalConf {
    image: String,
    use_for: Option<HashSet<QuantType>>,
    #[serde(default)]
    models: HashMap<String, LegacyBackendModelConf>,
}

#[derive(Deserialize, Debug, Default)]
struct LegacyBackendModelConf {
    #[serde(default)]
    environment_variables: HashMap<String, String>,
    #[serde(default)]
    arguments: Vec<String>,
    ctx_length: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct LegacyFactoryConf {
    runpod: Option<LegacyRunpodConf>,
    kubernetes: Option<LegacyKubernetesConf>,
}

#[derive(Deserialize, Debug)]
struct LegacyRunpodConf {
    token: Option<String>,
    template_id: Option<String>,
    gpu_memory_margin_factor: Option<f64>,
    disk_storage_margin_factor: Option<f64>,
    #[serde(default)]
    gpus: Vec<LegacyRunpodGpu>,
    #[serde(default)]
    models: HashMap<String, LegacyRunpodModel>,
}

#[derive(Deserialize, Debug)]
struct LegacyRunpodGpu {
    id: String,
    memory: u64,
    price: f64,
}

#[derive(Deserialize, Debug)]
struct LegacyRunpodModel {
    gpus: Option<Vec<LegacyRunpodGpu>>,
}

#[derive(Deserialize, Debug)]
struct LegacyKubernetesConf {
    namespace: Option<String>,
    pvc_name: Option<String>,
    cache_pvc_name: Option<String>,
    gpu_memory_margin_factor: Option<f64>,
    #[serde(default)]
    gpus: Vec<LegacyKubernetesGpu>,
    #[serde(default)]
    models: HashMap<String, LegacyKubernetesModel>,
}

#[derive(Deserialize, Debug)]
struct LegacyKubernetesGpu {
    id: String,
    memory: u64,
    max_count: Option<u64>,
}

#[derive(Deserialize, Debug)]
struct LegacyKubernetesModel {
    gpus: Option<Vec<LegacyKubernetesGpu>>,
}

fn generate_env_snippet(conf: &LegacyConf) -> String {
    let mut lines = Vec::new();
    if let Some(port) = conf.port {
        lines.push(format!("PORT={port}"));
    }
    if let Some(ref db_url) = conf.sqlite_url {
        lines.push(format!("DATABASE_URL={db_url}"));
    }
    if let Some(timeout) = conf.model_timeout {
        lines.push(format!("MODEL_TIMEOUT={timeout}"));
    }
    if let Some(retry) = conf.model_retry_interval {
        lines.push(format!("MODEL_RETRY_INTERVAL={retry}"));
    }
    if let Some(log_body) = conf.log_request_body {
        lines.push(format!("LOG_REQUEST_BODY={log_body}"));
    }
    if let Some(ref hf) = conf.huggingface
        && let Some(ref token) = hf.token
    {
        lines.push(format!("HF_TOKEN={token}"));
    }
    if let Some(ref tls) = conf.tls {
        match tls {
            LegacyTlsConfig::TlsPathConfig {
                cert_path,
                key_path,
            } => {
                lines.push(format!("TLS_CERT_PATH={cert_path}"));
                lines.push(format!("TLS_KEY_PATH={key_path}"));
            }
            LegacyTlsConfig::TlsInlineConfig { cert, key } => {
                lines.push(format!("TLS_CERT=\"{cert}\""));
                lines.push(format!("TLS_KEY=\"{key}\""));
            }
        }
    }
    if let Some(ref factory) = conf.factory {
        if let Some(ref runpod) = factory.runpod {
            lines.push("FACTORY_TYPE=runpod".to_string());
            if let Some(ref token) = runpod.token {
                lines.push(format!("RUNPOD_TOKEN={token}"));
            }
            if let Some(ref template_id) = runpod.template_id {
                lines.push(format!("RUNPOD_TEMPLATE_ID={template_id}"));
            }
            if let Some(margin) = runpod.gpu_memory_margin_factor {
                lines.push(format!("RUNPOD_GPU_MEMORY_MARGIN_FACTOR={margin}"));
            }
            if let Some(margin) = runpod.disk_storage_margin_factor {
                lines.push(format!("RUNPOD_DISK_STORAGE_MARGIN_FACTOR={margin}"));
            }
        } else if let Some(ref k8s) = factory.kubernetes {
            lines.push("FACTORY_TYPE=kubernetes".to_string());
            if let Some(ref ns) = k8s.namespace {
                lines.push(format!("KUBERNETES_NAMESPACE={ns}"));
            }
            if let Some(ref pvc) = k8s.pvc_name {
                lines.push(format!("KUBERNETES_PVC_NAME={pvc}"));
            }
            if let Some(ref cache_pvc) = k8s.cache_pvc_name {
                lines.push(format!("KUBERNETES_CACHE_PVC_NAME={cache_pvc}"));
            }
            if let Some(margin) = k8s.gpu_memory_margin_factor {
                lines.push(format!("KUBERNETES_GPU_MEMORY_MARGIN_FACTOR={margin}"));
            }
        }
    }

    lines.join("\n")
}

#[allow(clippy::too_many_lines)]
pub async fn check_and_migrate_legacy_config() -> Result<(), anyhow::Error> {
    let toml_path = Path::new("config.toml");
    if !toml_path.exists() {
        return Ok(());
    }

    let content = match std::fs::read_to_string(toml_path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read config.toml: {e}");
            return Ok(());
        }
    };

    let legacy_conf: LegacyConf = match toml::from_str(&content) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to parse legacy config.toml for migration: {e}");
            return Ok(());
        }
    };

    let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(get_executor())
        .await
        .unwrap_or(0);
    let model_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM models")
        .fetch_one(get_executor())
        .await
        .unwrap_or(0);

    let is_empty = user_count == 0 && model_count == 0;
    let env_snippet = generate_env_snippet(&legacy_conf);

    if !is_empty {
        println!(
            "\n================================================================================"
        );
        println!("NOTICE: Legacy 'config.toml' was found, but the database already contains data.");
        println!(
            "The database was NOT modified. 'config.toml' is discontinued and will be ignored."
        );
        println!("Suggested environment variables matching 'config.toml':\n");
        println!("{env_snippet}\n");
        println!("Please remove or rename 'config.toml' to suppress this notice.");
        println!(
            "================================================================================\n"
        );
        return Ok(());
    }

    info!("Database is empty. Migrating legacy configuration from config.toml into database...");

    // 1. Users & Tokens
    for (username, user) in &legacy_conf.users {
        create_user(
            username,
            user.limit_to_models.as_ref(),
            user.log_request_body,
        )
        .await?;
        for hash in &user.token_hashes {
            add_user_token(hash, username).await?;
        }
    }

    // 2. OpenAI APIs
    for (api_id, api) in &legacy_conf.openai_apis {
        create_openai_api(
            api_id,
            api.url.as_str(),
            api.token.as_deref(),
            api.prefix.as_deref(),
            api.models.as_ref(),
            api.timeout,
            api.headers.as_ref(),
        )
        .await?;
    }

    // 3. Models
    for (model_id, model) in &legacy_conf.models {
        let handler = match &model.handler {
            Some(LegacyHandler::OpenAi(openai)) => {
                ModelHandlerConfig::OpenAi(OpenAiHandlerConfig {
                    api: openai.api.clone(),
                    url: openai.url.clone(),
                    token: openai
                        .token
                        .as_ref()
                        .map(|t| SecretBox::new(Box::new(t.clone()))),
                    model: openai.model.clone(),
                    timeout: openai.timeout,
                    headers: openai.headers.clone(),
                })
            }
            Some(LegacyHandler::Spawner(spawner)) => {
                ModelHandlerConfig::Spawner(SpawnerHandlerConfig {
                    backends: spawner.backends.clone(),
                    min_scale: spawner.min_scale.unwrap_or(0),
                    max_scale: spawner.max_scale.unwrap_or(1),
                    queue_length_for_scale_up: spawner.queue_length_for_scale_up.unwrap_or(1),
                })
            }
            None => ModelHandlerConfig::Spawner(SpawnerHandlerConfig::default()),
        };

        let model_config = ModelConfig {
            id: model_id.clone(),
            name: model.name.clone(),
            timeout: model.timeout,
            limit_to_users: model.limit_to_users.clone(),
            hidden: model.hidden,
            handler,
        };
        create_model(&model_config).await?;
    }

    // 4. Aliases
    for (alias, target) in &legacy_conf.aliases {
        create_alias(alias, &target.to).await?;
    }

    // 5. Backends
    for (backend_id, backend) in &legacy_conf.backends {
        let (backend_type, image, use_for, models) = match backend {
            LegacyBackendConf::Tgi(b) => ("tgi", &b.image, &b.use_for, &b.models),
            LegacyBackendConf::Vllm(b) => ("vllm", &b.image, &b.use_for, &b.models),
            LegacyBackendConf::LlamaCpp(b) => ("llama_cpp", &b.image, &b.use_for, &b.models),
            LegacyBackendConf::Sglang(b) => ("sglang", &b.image, &b.use_for, &b.models),
        };

        create_backend(backend_id, backend_type, image, use_for.as_ref()).await?;

        for (model_name, model_conf) in models {
            create_backend_model_config(
                backend_id,
                model_name,
                &model_conf.arguments,
                &model_conf.environment_variables,
                model_conf.ctx_length,
            )
            .await?;
        }
    }

    // 6. Factory GPUs
    if let Some(ref factory) = legacy_conf.factory {
        if let Some(ref runpod) = factory.runpod {
            for gpu in &runpod.gpus {
                create_factory_gpu("runpod", &gpu.id, gpu.memory, Some(gpu.price), None, None)
                    .await?;
            }
            for (model_name, model_conf) in &runpod.models {
                if let Some(ref gpus) = model_conf.gpus {
                    for gpu in gpus {
                        create_factory_gpu(
                            "runpod",
                            &gpu.id,
                            gpu.memory,
                            Some(gpu.price),
                            None,
                            Some(model_name),
                        )
                        .await?;
                    }
                }
            }
        }
        if let Some(ref k8s) = factory.kubernetes {
            for gpu in &k8s.gpus {
                create_factory_gpu("kubernetes", &gpu.id, gpu.memory, None, gpu.max_count, None)
                    .await?;
            }
            for (model_name, model_conf) in &k8s.models {
                if let Some(ref gpus) = model_conf.gpus {
                    for gpu in gpus {
                        create_factory_gpu(
                            "kubernetes",
                            &gpu.id,
                            gpu.memory,
                            None,
                            gpu.max_count,
                            Some(model_name),
                        )
                        .await?;
                    }
                }
            }
        }
    }

    println!("\n================================================================================");
    println!("AUTOMATIC CONFIGURATION MIGRATION SUCCESSFUL");
    println!("================================================================================");
    println!(
        "Imported users, models, backends, aliases, and GPUs from 'config.toml' into the database.\n"
    );
    println!(
        "To complete migration, set the following environment variables (e.g. in your .env):\n"
    );
    println!("{env_snippet}\n");
    println!("Once configured, please remove or rename 'config.toml'.");
    println!("================================================================================\n");

    Ok(())
}
