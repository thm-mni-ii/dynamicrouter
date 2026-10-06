use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

// --------------------------------------------------------------------------
// Auth & User Models
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct UserProfile {
    pub username: String,
    pub email: Option<String>,
    pub is_admin: bool,
    pub limit_to_models: Option<HashSet<String>>,
    pub log_request_body: Option<bool>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthConfig {
    pub oidc_enabled: bool,
    pub issuer: Option<String>,
    pub client_id: Option<String>,
    pub redirect_uri: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UserToken {
    pub token_hash: String,
    pub created_at: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CreatedTokenResponse {
    pub token: String,
    pub username: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AdminUserRecord {
    pub username: String,
    pub limit_to_models: Option<HashSet<String>>,
    pub log_request_body: Option<bool>,
    pub is_admin: bool,
}

// --------------------------------------------------------------------------
// Models & Routing
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelListItem {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelsListResponse {
    pub data: Vec<ModelListItem>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SpawnerConfig {
    pub backends: Option<HashSet<String>>,
    pub min_scale: u32,
    pub max_scale: u32,
    pub queue_length_for_scale_up: u32,
}

impl Default for SpawnerConfig {
    fn default() -> Self {
        Self {
            backends: None,
            min_scale: 0,
            max_scale: 1,
            queue_length_for_scale_up: 1,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct OpenAiHandlerConf {
    pub api: Option<String>,
    pub url: Option<String>,
    pub token: Option<String>,
    pub model: Option<String>,
    pub timeout: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ModelHandlerConfig {
    #[serde(rename = "spawner")]
    Spawner(SpawnerConfig),
    #[serde(rename = "openai")]
    OpenAi(OpenAiHandlerConf),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AdminModelConfig {
    pub id: String,
    pub name: Option<String>,
    pub timeout: Option<u32>,
    pub limit_to_users: Option<HashSet<String>>,
    pub hidden: Option<bool>,
    pub handler: ModelHandlerConfig,
}

// --------------------------------------------------------------------------
// OpenAI Upstream APIs
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OpenAiApiRecord {
    pub id: String,
    pub url: String,
    pub token: Option<String>,
    pub prefix: Option<String>,
    pub models: Option<HashSet<String>>,
    pub timeout: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

// --------------------------------------------------------------------------
// Backends & Model Arguments
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BackendUseFor(pub Option<HashSet<String>>);

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BackendRecord {
    pub id: String,
    pub backend_type: String,
    pub image: String,
    pub use_for: BackendUseFor,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BackendModelRecord {
    pub backend_id: String,
    pub model: String,
    pub arguments: Vec<String>,
    pub environment_variables: HashMap<String, String>,
    pub ctx_length: Option<u64>,
}

// --------------------------------------------------------------------------
// Factory GPUs & Instances & Aliases
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FactoryGpuRecord {
    pub factory_type: String,
    pub id: String,
    pub memory: i64,
    pub price: f64,
    pub max_count: i64,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct InstanceSummary {
    pub id: String,
    pub model: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct AliasRecord {
    pub alias: String,
    pub target: String,
}

// --------------------------------------------------------------------------
// Request Logs & Inspector
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RequestLog {
    pub id: String,
    pub model: String,
    pub enqueue: i64,
    pub done: Option<i64>,
    pub user: Option<String>,
    pub instance: Option<String>,
    pub request_body: Option<serde_json::Value>,
    pub response_body: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PaginatedRequests {
    pub requests: Vec<RequestLog>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

// --------------------------------------------------------------------------
// Metrics
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct MetricsSummary {
    pub total_requests: i64,
    pub pending_requests: i64,
    pub active_instances: i64,
    pub avg_duration_secs: Option<f64>,
    pub p50_duration_secs: Option<f64>,
    pub p99_duration_secs: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModelMetric {
    pub model: String,
    pub total_requests: i64,
    pub active_instances: i64,
    pub avg_duration_secs: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct UserMetric {
    pub user: String,
    pub total_requests: i64,
    pub avg_duration_secs: Option<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TimeseriesPoint {
    pub timestamp: i64,
    pub count: i64,
}

// --------------------------------------------------------------------------
// Rate Limits & Quotas
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RateLimitRecord {
    pub id: String,
    pub user: Option<String>,
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: String,
    pub created_at: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct RateLimitUsage {
    pub id: String,
    pub user: Option<String>,
    pub model: Option<String>,
    pub time_window: String,
    pub limit_value: i64,
    pub unit: String,
    pub used: i64,
    pub remaining: i64,
    pub reset_in_seconds: Option<i64>,
    pub reset_timestamp: Option<i64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RateLimitListResponse {
    pub rate_limits: Vec<RateLimitRecord>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RateLimitUsageListResponse {
    pub usages: Vec<RateLimitUsage>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MyRateLimitsResponse {
    pub rate_limits: Vec<RateLimitUsage>,
}

// --------------------------------------------------------------------------
// Chat API Payloads
// --------------------------------------------------------------------------
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatChoice {
    pub message: ChatMessage,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ChatCompletionResponse {
    pub choices: Vec<ChatChoice>,
}
