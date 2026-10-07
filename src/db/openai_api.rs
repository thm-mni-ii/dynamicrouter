use crate::db::connection::get_executor;
use secrecy::{ExposeSecret, SecretBox};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use url::Url;

pub mod secret_serde {
    use secrecy::{ExposeSecret, SecretBox};
    use serde::{Deserializer, Serializer};

    #[allow(clippy::ref_option)]
    pub fn serialize<S>(
        secret: &Option<SecretBox<String>>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match secret {
            Some(s) => serializer.serialize_some(s.expose_secret()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<SecretBox<String>>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = serde::Deserialize::deserialize(deserializer)?;
        Ok(opt.map(|s| SecretBox::new(Box::new(s))))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OpenAiApiRecord {
    pub id: String,
    pub url: Url,
    #[serde(default, with = "secret_serde")]
    pub token: Option<SecretBox<String>>,
    pub prefix: Option<String>,
    pub models: Option<HashSet<String>>,
    pub timeout: Option<u32>,
    pub headers: Option<HashMap<String, String>>,
}

impl Clone for OpenAiApiRecord {
    fn clone(&self) -> Self {
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
    /// Validates an endpoint base URL and returns an overridable suggestion if a provider-specific prefix is missing.
    pub fn suggest_corrected_url(url_str: &str) -> Option<(&'static str, &'static str)> {
        let trimmed = url_str.trim();
        let parsed = Url::parse(trimmed).ok()?;
        let host = parsed.host_str()?;
        let path = parsed.path().trim_end_matches('/');

        if host == "openrouter.ai" || host.ends_with(".openrouter.ai") {
            if path.is_empty() || path == "/" {
                return Some((
                    "https://openrouter.ai/api/v1",
                    "OpenRouter API requires the '/api/v1' path prefix (e.g. https://openrouter.ai/api/v1).",
                ));
            } else if path == "/api" {
                return Some((
                    "https://openrouter.ai/api/v1",
                    "OpenRouter API requires the '/api/v1' path prefix.",
                ));
            }
        } else if host == "api.groq.com" || host.ends_with(".groq.com") {
            if path.is_empty() || path == "/" {
                return Some((
                    "https://api.groq.com/openai/v1",
                    "Groq OpenAI API requires the '/openai/v1' path prefix.",
                ));
            } else if path == "/openai" {
                return Some((
                    "https://api.groq.com/openai/v1",
                    "Groq OpenAI API requires the '/openai/v1' path prefix.",
                ));
            }
        } else if host == "api.deepinfra.com" || host.ends_with(".deepinfra.com") {
            if path.is_empty() || path == "/" || path == "/v1" {
                return Some((
                    "https://api.deepinfra.com/v1/openai",
                    "DeepInfra OpenAI API requires the '/v1/openai' path prefix.",
                ));
            }
        } else if (host == "api.openai.com" || host.ends_with(".openai.com")) && (path.is_empty() || path == "/") {
            return Some((
                "https://api.openai.com/v1",
                "OpenAI API typically uses the '/v1' path prefix.",
            ));
        }

        None
    }

    pub fn endpoint_url(&self, path: &str) -> Result<Url, url::ParseError> {
        let clean_path = path.trim_start_matches('/');
        let raw_url = self.url.as_str().trim_end_matches('/');

        // If the URL already ends with the target endpoint (e.g. "chat/completions"), use it directly
        if raw_url.ends_with(clean_path) {
            return Url::parse(raw_url);
        }

        let host = self.url.host_str().unwrap_or("");
        let path_part = self.url.path().trim_end_matches('/');

        let base = if (host == "openrouter.ai" || host.ends_with(".openrouter.ai"))
            && (path_part.is_empty() || path_part == "/" || path_part == "/api")
        {
            format!("{}/api/v1", self.url.origin().ascii_serialization())
        } else if (host == "api.groq.com" || host.ends_with(".groq.com"))
            && (path_part.is_empty() || path_part == "/" || path_part == "/openai")
        {
            format!("{}/openai/v1", self.url.origin().ascii_serialization())
        } else if (host == "api.deepinfra.com" || host.ends_with(".deepinfra.com"))
            && (path_part.is_empty() || path_part == "/" || path_part == "/v1")
        {
            format!("{}/v1/openai", self.url.origin().ascii_serialization())
        } else {
            let mut b = raw_url.to_string();
            if !b.ends_with("/v1") && !b.contains("/v1/") && !b.ends_with("/openai") {
                b.push_str("/v1");
            }
            b
        };

        let base_clean = base.trim_end_matches('/');
        let url_str = format!("{base_clean}/{clean_path}");
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

pub async fn delete_openai_api(id: &str) -> Result<bool, anyhow::Error> {
    let res = sqlx::query("DELETE FROM openai_apis WHERE id = ?")
        .bind(id)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}
