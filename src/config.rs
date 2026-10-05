use once_cell::sync::Lazy;
use secrecy::SecretBox;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FactoryType {
    Runpod,
    Kubernetes,
}

#[derive(Debug, Clone)]
pub enum TlsConfig {
    TlsPathConfig { cert_path: String, key_path: String },
    TlsInlineConfig { cert: String, key: String },
}

#[derive(Debug)]
pub struct EnvConfig {
    pub port: u16,
    pub host: String,
    pub database_url: String,
    pub tls: Option<TlsConfig>,
    pub model_timeout: u32,
    pub model_retry_interval: u32,
    pub log_request_body: bool,
    pub huggingface_token: Option<SecretBox<String>>,
    pub factory_type: Option<FactoryType>,
    pub runpod_token: Option<SecretBox<String>>,
    pub runpod_template_id: Option<String>,
    pub runpod_gpu_memory_margin_factor: f64,
    pub runpod_disk_storage_margin_factor: f64,
    pub kubernetes_namespace: String,
    pub kubernetes_pvc_name: String,
    pub kubernetes_cache_pvc_name: Option<String>,
    pub kubernetes_gpu_memory_margin_factor: f64,
}

impl EnvConfig {
    pub fn from_env() -> Self {
        let port = env::var("PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(3000);

        let host = env::var("HOST")
            .or_else(|_| env::var("BIND_ADDR"))
            .unwrap_or_else(|_| "0.0.0.0".to_string());

        let database_url = env::var("DATABASE_URL")
            .or_else(|_| env::var("SQLITE_URL"))
            .unwrap_or_else(|_| "sqlite://dynamicrouter.db".to_string());

        let tls = if let (Ok(cert_path), Ok(key_path)) =
            (env::var("TLS_CERT_PATH"), env::var("TLS_KEY_PATH"))
        {
            Some(TlsConfig::TlsPathConfig {
                cert_path,
                key_path,
            })
        } else if let (Ok(cert), Ok(key)) = (env::var("TLS_CERT"), env::var("TLS_KEY")) {
            Some(TlsConfig::TlsInlineConfig { cert, key })
        } else {
            None
        };

        let model_timeout = env::var("MODEL_TIMEOUT")
            .ok()
            .and_then(|t| t.parse().ok())
            .unwrap_or(900);

        let model_retry_interval = env::var("MODEL_RETRY_INTERVAL")
            .ok()
            .and_then(|r| r.parse().ok())
            .unwrap_or(60);

        let log_request_body = env::var("LOG_REQUEST_BODY")
            .is_ok_and(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes"));

        let huggingface_token = env::var("HF_TOKEN")
            .or_else(|_| env::var("HUGGINGFACE_TOKEN"))
            .ok()
            .map(|t| SecretBox::new(Box::new(t)));

        let runpod_token = env::var("RUNPOD_TOKEN")
            .or_else(|_| env::var("RUNPOD_API_KEY"))
            .ok()
            .map(|t| SecretBox::new(Box::new(t)));

        let factory_type = match env::var("FACTORY_TYPE").ok().as_deref() {
            Some("runpod") => Some(FactoryType::Runpod),
            Some("kubernetes") => Some(FactoryType::Kubernetes),
            _ => {
                if runpod_token.is_some() {
                    Some(FactoryType::Runpod)
                } else if env::var("KUBERNETES_SERVICE_HOST").is_ok()
                    || env::var("KUBERNETES_PORT").is_ok()
                {
                    Some(FactoryType::Kubernetes)
                } else {
                    None
                }
            }
        };

        let runpod_template_id = env::var("RUNPOD_TEMPLATE_ID")
            .ok()
            .or_else(|| Some("runpod-torch-v240".to_string()));

        let runpod_gpu_memory_margin_factor = env::var("RUNPOD_GPU_MEMORY_MARGIN_FACTOR")
            .ok()
            .and_then(|f| f.parse().ok())
            .unwrap_or(1.2);

        let runpod_disk_storage_margin_factor = env::var("RUNPOD_DISK_STORAGE_MARGIN_FACTOR")
            .ok()
            .and_then(|f| f.parse().ok())
            .unwrap_or(2.0);

        let kubernetes_namespace =
            env::var("KUBERNETES_NAMESPACE").unwrap_or_else(|_| "dynamicrouter".to_string());

        let kubernetes_pvc_name =
            env::var("KUBERNETES_PVC_NAME").unwrap_or_else(|_| "dynamicrouter-models".to_string());

        let kubernetes_cache_pvc_name = env::var("KUBERNETES_CACHE_PVC_NAME").ok();

        let kubernetes_gpu_memory_margin_factor = env::var("KUBERNETES_GPU_MEMORY_MARGIN_FACTOR")
            .ok()
            .and_then(|f| f.parse().ok())
            .unwrap_or(1.2);

        EnvConfig {
            port,
            host,
            database_url,
            tls,
            model_timeout,
            model_retry_interval,
            log_request_body,
            huggingface_token,
            factory_type,
            runpod_token,
            runpod_template_id,
            runpod_gpu_memory_margin_factor,
            runpod_disk_storage_margin_factor,
            kubernetes_namespace,
            kubernetes_pvc_name,
            kubernetes_cache_pvc_name,
            kubernetes_gpu_memory_margin_factor,
        }
    }

    pub fn should_log_request_body_for_user(&self, user_override: Option<bool>) -> bool {
        user_override.unwrap_or(self.log_request_body)
    }
}

pub static CONFIG: Lazy<EnvConfig> = Lazy::new(EnvConfig::from_env);
