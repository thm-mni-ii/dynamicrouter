use crate::db::connection::get_executor;
use secrecy::SecretBox;
use std::collections::{HashMap, HashSet};
use url::Url;

#[derive(Debug)]
pub struct OpenAiApiRecord {
    pub id: String,
    pub url: Url,
    pub token: Option<SecretBox<String>>,
    pub prefix: Option<String>,
    pub models: Option<HashSet<String>>,
    pub timeout: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

impl Clone for OpenAiApiRecord {
    fn clone(&self) -> Self {
        use secrecy::ExposeSecret;
        OpenAiApiRecord {
            id: self.id.clone(),
            url: self.url.clone(),
            token: self
                .token
                .as_ref()
                .map(|t| SecretBox::new(Box::new(t.expose_secret().clone()))),
            prefix: self.prefix.clone(),
            models: self.models.clone(),
            timeout: self.timeout,
            headers: self.headers.clone(),
        }
    }
}

impl OpenAiApiRecord {
    pub fn endpoint_url(&self, path: &str) -> Result<Url, url::ParseError> {
        let mut base = self.url.as_str().trim_end_matches('/').to_string();
        if !base.ends_with("/v1") && !base.contains("/v1/") {
            base.push_str("/v1");
        }
        let clean_path = path.trim_start_matches('/');
        let url_str = format!("{base}/{clean_path}");
        Url::parse(&url_str)
    }

    pub fn map_model_name(&self, model_name: &str) -> String {
        if let Some(prefix) = &self.prefix {
            let p = prefix.trim_end_matches('/');
            if let Some(stripped) = model_name.strip_prefix(prefix.as_str()) {
                stripped.trim_start_matches('/').to_string()
            } else if let Some(stripped) = model_name.strip_prefix(p) {
                stripped.trim_start_matches('/').to_string()
            } else {
                model_name.to_string()
            }
        } else {
            model_name.to_string()
        }
    }

    pub fn matches_model(&self, model_name: &str) -> bool {
        if let Some(prefix) = &self.prefix {
            let p = prefix.trim_end_matches('/');
            if model_name.starts_with(prefix.as_str())
                || model_name.starts_with(&format!("{p}/"))
                || model_name == p
            {
                return true;
            }
        }
        if let Some(models) = &self.models {
            if models.contains(model_name) {
                return true;
            }
            if self.prefix.is_some() {
                let stripped = self.map_model_name(model_name);
                if models.contains(&stripped) {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(sqlx::FromRow)]
struct OpenAiApiRow {
    id: String,
    url: String,
    token: Option<String>,
    prefix: Option<String>,
    models: Option<String>,
    timeout: Option<i64>,
    headers: Option<String>,
}

impl OpenAiApiRow {
    fn into_record(self) -> Result<OpenAiApiRecord, anyhow::Error> {
        let url = Url::parse(&self.url)?;
        let models = self
            .models
            .and_then(|s| serde_json::from_str::<HashSet<String>>(&s).ok());
        let headers = self
            .headers
            .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok());
        let timeout = self.timeout.and_then(|t| u32::try_from(t).ok());
        let token = self.token.map(|t| SecretBox::new(Box::new(t)));

        Ok(OpenAiApiRecord {
            id: self.id,
            url,
            token,
            prefix: self.prefix,
            models,
            timeout,
            headers,
        })
    }
}

pub async fn get_all_openai_apis() -> Result<Vec<OpenAiApiRecord>, anyhow::Error> {
    let rows = sqlx::query_as::<_, OpenAiApiRow>(
        r"
        SELECT id, url, token, prefix, models, timeout, headers
        FROM openai_apis
        ORDER BY id
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let mut records = Vec::new();
    for row in rows {
        if let Ok(rec) = row.into_record() {
            records.push(rec);
        }
    }
    Ok(records)
}

pub async fn get_openai_api(id: &str) -> Result<Option<OpenAiApiRecord>, anyhow::Error> {
    let row = sqlx::query_as::<_, OpenAiApiRow>(
        r"
        SELECT id, url, token, prefix, models, timeout, headers
        FROM openai_apis
        WHERE id = ?
        ",
    )
    .bind(id)
    .fetch_optional(get_executor())
    .await?;

    match row {
        Some(r) => Ok(Some(r.into_record()?)),
        None => Ok(None),
    }
}

pub async fn create_openai_api(
    id: &str,
    url: &str,
    token: Option<&str>,
    prefix: Option<&str>,
    models: Option<&HashSet<String>>,
    timeout: Option<u32>,
    headers: Option<&HashMap<String, String>>,
) -> Result<(), anyhow::Error> {
    let models_str = models.map(|m| serde_json::to_string(m).unwrap_or_default());
    let headers_str = headers.map(|h| serde_json::to_string(h).unwrap_or_default());
    let timeout_i64 = timeout.map(i64::from);

    sqlx::query(
        r"
        INSERT INTO openai_apis (id, url, token, prefix, models, timeout, headers)
        VALUES (?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            url = excluded.url,
            token = excluded.token,
            prefix = excluded.prefix,
            models = excluded.models,
            timeout = excluded.timeout,
            headers = excluded.headers
        ",
    )
    .bind(id)
    .bind(url)
    .bind(token)
    .bind(prefix)
    .bind(models_str)
    .bind(timeout_i64)
    .bind(headers_str)
    .execute(get_executor())
    .await?;
    Ok(())
}
