use crate::db::connection::get_executor;
use secrecy::SecretBox;
use std::collections::{HashMap, HashSet};
use url::Url;

#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub id: String,
    pub name: Option<String>,
    pub timeout: Option<u32>,
    pub limit_to_users: Option<HashSet<String>>,
    pub hidden: Option<bool>,
    pub handler: ModelHandlerConfig,
}

#[derive(Debug, Clone)]
pub enum ModelHandlerConfig {
    Spawner(SpawnerHandlerConfig),
    OpenAi(OpenAiHandlerConfig),
}

impl ModelHandlerConfig {
    pub fn as_spawner(&self) -> &SpawnerHandlerConfig {
        match self {
            ModelHandlerConfig::Spawner(conf) => conf,
            _ => panic!("model handler config is not spawner"),
        }
    }

    #[allow(dead_code)]
    pub fn as_openai(&self) -> &OpenAiHandlerConfig {
        match self {
            ModelHandlerConfig::OpenAi(conf) => conf,
            _ => panic!("model handler config is not openai"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SpawnerHandlerConfig {
    pub backends: Option<HashSet<String>>,
    pub min_scale: u32,
    pub max_scale: u32,
    pub queue_length_for_scale_up: u32,
}

impl Default for SpawnerHandlerConfig {
    fn default() -> Self {
        SpawnerHandlerConfig {
            backends: None,
            min_scale: 0,
            max_scale: 1,
            queue_length_for_scale_up: 1,
        }
    }
}

#[derive(Debug, Default)]
pub struct OpenAiHandlerConfig {
    pub api: Option<String>,
    pub url: Option<Url>,
    pub token: Option<SecretBox<String>>,
    pub model: Option<String>,
    pub timeout: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

impl Clone for OpenAiHandlerConfig {
    fn clone(&self) -> Self {
        use secrecy::ExposeSecret;
        OpenAiHandlerConfig {
            api: self.api.clone(),
            url: self.url.clone(),
            token: self
                .token
                .as_ref()
                .map(|t| SecretBox::new(Box::new(t.expose_secret().clone()))),
            model: self.model.clone(),
            timeout: self.timeout,
            headers: self.headers.clone(),
        }
    }
}

#[derive(sqlx::FromRow)]
struct ModelRow {
    id: String,
    name: Option<String>,
    timeout: Option<i64>,
    limit_to_users: Option<String>,
    hidden: Option<bool>,
    handler_type: String,
    spawner_backends: Option<String>,
    min_scale: i64,
    max_scale: i64,
    queue_length_for_scale_up: i64,
    openai_api_id: Option<String>,
    openai_url: Option<String>,
    openai_token: Option<String>,
    openai_model: Option<String>,
    openai_headers: Option<String>,
    openai_timeout: Option<i64>,
}

impl ModelRow {
    fn into_model_config(self) -> Result<ModelConfig, anyhow::Error> {
        let limit_to_users = self
            .limit_to_users
            .and_then(|s| serde_json::from_str::<HashSet<String>>(&s).ok());
        let timeout = self.timeout.and_then(|t| u32::try_from(t).ok());

        let handler = if self.handler_type.as_str() == "openai" {
            let url = self.openai_url.as_deref().map(Url::parse).transpose()?;
            let headers = self
                .openai_headers
                .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok());
            let token = self.openai_token.map(|t| SecretBox::new(Box::new(t)));
            let openai_timeout = self.openai_timeout.and_then(|t| u32::try_from(t).ok());

            ModelHandlerConfig::OpenAi(OpenAiHandlerConfig {
                api: self.openai_api_id,
                url,
                token,
                model: self.openai_model,
                timeout: openai_timeout,
                headers,
            })
        } else {
            let backends = self
                .spawner_backends
                .and_then(|s| serde_json::from_str::<HashSet<String>>(&s).ok());
            let min_scale = u32::try_from(self.min_scale).unwrap_or(0);
            let max_scale = u32::try_from(self.max_scale).unwrap_or(1);
            let queue_length_for_scale_up =
                u32::try_from(self.queue_length_for_scale_up).unwrap_or(1);

            ModelHandlerConfig::Spawner(SpawnerHandlerConfig {
                backends,
                min_scale,
                max_scale,
                queue_length_for_scale_up,
            })
        };

        Ok(ModelConfig {
            id: self.id,
            name: self.name,
            timeout,
            limit_to_users,
            hidden: self.hidden,
            handler,
        })
    }
}

pub async fn get_model_config(model_id: &str) -> Result<Option<ModelConfig>, anyhow::Error> {
    if let Some(config) = get_exact_model_config(model_id).await? {
        return Ok(Some(config));
    }
    if model_id != "*" {
        return get_exact_model_config("*").await;
    }
    Ok(None)
}

pub async fn get_exact_model_config(model_id: &str) -> Result<Option<ModelConfig>, anyhow::Error> {
    let row = sqlx::query_as::<_, ModelRow>(
        r"
        SELECT
            id, name, timeout, limit_to_users, hidden, handler_type,
            spawner_backends, min_scale, max_scale, queue_length_for_scale_up,
            openai_api_id, openai_url, openai_token, openai_model, openai_headers, openai_timeout
        FROM models
        WHERE id = ?
        ",
    )
    .bind(model_id)
    .fetch_optional(get_executor())
    .await?;

    match row {
        Some(r) => Ok(Some(r.into_model_config()?)),
        None => Ok(None),
    }
}

pub async fn get_all_models() -> Result<Vec<ModelConfig>, anyhow::Error> {
    let rows = sqlx::query_as::<_, ModelRow>(
        r"
        SELECT
            id, name, timeout, limit_to_users, hidden, handler_type,
            spawner_backends, min_scale, max_scale, queue_length_for_scale_up,
            openai_api_id, openai_url, openai_token, openai_model, openai_headers, openai_timeout
        FROM models
        ORDER BY id
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let mut list = Vec::new();
    for row in rows {
        if let Ok(config) = row.into_model_config() {
            list.push(config);
        }
    }
    Ok(list)
}

pub async fn create_model(model: &ModelConfig) -> Result<(), anyhow::Error> {
    let limit_str = model
        .limit_to_users
        .as_ref()
        .map(|l| serde_json::to_string(l).unwrap_or_default());
    let timeout_i64 = model.timeout.map(i64::from);

    match &model.handler {
        ModelHandlerConfig::Spawner(spawner) => {
            let backends_str = spawner
                .backends
                .as_ref()
                .map(|b| serde_json::to_string(b).unwrap_or_default());
            let min_scale = i64::from(spawner.min_scale);
            let max_scale = i64::from(spawner.max_scale);
            let queue_len = i64::from(spawner.queue_length_for_scale_up);

            sqlx::query(
                r"
                INSERT INTO models (
                    id, name, timeout, limit_to_users, hidden, handler_type,
                    spawner_backends, min_scale, max_scale, queue_length_for_scale_up
                )
                VALUES (?, ?, ?, ?, ?, 'spawner', ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    timeout = excluded.timeout,
                    limit_to_users = excluded.limit_to_users,
                    hidden = excluded.hidden,
                    handler_type = 'spawner',
                    spawner_backends = excluded.spawner_backends,
                    min_scale = excluded.min_scale,
                    max_scale = excluded.max_scale,
                    queue_length_for_scale_up = excluded.queue_length_for_scale_up
                ",
            )
            .bind(&model.id)
            .bind(&model.name)
            .bind(timeout_i64)
            .bind(limit_str)
            .bind(model.hidden)
            .bind(backends_str)
            .bind(min_scale)
            .bind(max_scale)
            .bind(queue_len)
            .execute(get_executor())
            .await?;
        }
        ModelHandlerConfig::OpenAi(openai) => {
            use secrecy::ExposeSecret;
            let url_str = openai.url.as_ref().map(ToString::to_string);
            let token_str = openai.token.as_ref().map(|t| t.expose_secret().clone());
            let headers_str = openai
                .headers
                .as_ref()
                .map(|h| serde_json::to_string(h).unwrap_or_default());
            let openai_timeout_i64 = openai.timeout.map(i64::from);

            sqlx::query(
                r"
                INSERT INTO models (
                    id, name, timeout, limit_to_users, hidden, handler_type,
                    openai_api_id, openai_url, openai_token, openai_model, openai_headers, openai_timeout
                )
                VALUES (?, ?, ?, ?, ?, 'openai', ?, ?, ?, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    name = excluded.name,
                    timeout = excluded.timeout,
                    limit_to_users = excluded.limit_to_users,
                    hidden = excluded.hidden,
                    handler_type = 'openai',
                    openai_api_id = excluded.openai_api_id,
                    openai_url = excluded.openai_url,
                    openai_token = excluded.openai_token,
                    openai_model = excluded.openai_model,
                    openai_headers = excluded.openai_headers,
                    openai_timeout = excluded.openai_timeout
                ",
            )
            .bind(&model.id)
            .bind(&model.name)
            .bind(timeout_i64)
            .bind(limit_str)
            .bind(model.hidden)
            .bind(&openai.api)
            .bind(url_str)
            .bind(token_str)
            .bind(&openai.model)
            .bind(headers_str)
            .bind(openai_timeout_i64)
            .execute(get_executor())
            .await?;
        }
    }
    Ok(())
}
