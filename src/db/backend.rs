use crate::db::connection::get_executor;
use crate::utils::QuantType;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BackendUseFor(pub Option<HashSet<QuantType>>);

impl BackendUseFor {
    pub fn should_be_used_for(&self, quant_type: QuantType) -> bool {
        match &self.0 {
            None => true,
            Some(quants) => quants.contains(&quant_type),
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackendRecord {
    pub id: String,
    pub backend_type: String,
    pub image: String,
    pub use_for: BackendUseFor,
}

#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct BackendModelRecord {
    pub backend_id: String,
    pub model: String,
    pub arguments: Vec<String>,
    pub environment_variables: HashMap<String, String>,
    pub ctx_length: Option<u64>,
}

#[derive(sqlx::FromRow)]
struct BackendRow {
    id: String,
    backend_type: String,
    image: String,
    use_for: Option<String>,
}

impl BackendRow {
    fn into_record(self) -> BackendRecord {
        let use_for = self
            .use_for
            .and_then(|s| serde_json::from_str::<HashSet<QuantType>>(&s).ok());
        BackendRecord {
            id: self.id,
            backend_type: self.backend_type,
            image: self.image,
            use_for: BackendUseFor(use_for),
        }
    }
}

#[derive(sqlx::FromRow)]
struct BackendModelRow {
    backend_id: String,
    model: String,
    arguments: Option<String>,
    environment_variables: Option<String>,
    ctx_length: Option<i64>,
}

impl BackendModelRow {
    fn into_record(self) -> BackendModelRecord {
        let arguments = self
            .arguments
            .and_then(|s| serde_json::from_str::<Vec<String>>(&s).ok())
            .unwrap_or_default();
        let environment_variables = self
            .environment_variables
            .and_then(|s| serde_json::from_str::<HashMap<String, String>>(&s).ok())
            .unwrap_or_default();
        let ctx_length = self.ctx_length.and_then(|c| u64::try_from(c).ok());

        BackendModelRecord {
            backend_id: self.backend_id,
            model: self.model,
            arguments,
            environment_variables,
            ctx_length,
        }
    }
}

pub async fn get_all_backends() -> Result<Vec<BackendRecord>, anyhow::Error> {
    let rows = sqlx::query_as::<_, BackendRow>(
        r"
        SELECT id, backend_type, image, use_for
        FROM backends
        ORDER BY id
        ",
    )
    .fetch_all(get_executor())
    .await?;

    Ok(rows.into_iter().map(BackendRow::into_record).collect())
}

#[allow(dead_code)]
pub async fn get_backend(id: &str) -> Result<Option<BackendRecord>, anyhow::Error> {
    let row = sqlx::query_as::<_, BackendRow>(
        r"
        SELECT id, backend_type, image, use_for
        FROM backends
        WHERE id = ?
        ",
    )
    .bind(id)
    .fetch_optional(get_executor())
    .await?;

    Ok(row.map(BackendRow::into_record))
}

pub async fn get_backend_model_config(
    backend_id: &str,
    model: &str,
) -> Result<Option<BackendModelRecord>, anyhow::Error> {
    let row = sqlx::query_as::<_, BackendModelRow>(
        r"
        SELECT backend_id, model, arguments, environment_variables, ctx_length
        FROM backend_model_configs
        WHERE backend_id = ? AND model = ?
        ",
    )
    .bind(backend_id)
    .bind(model)
    .fetch_optional(get_executor())
    .await?;

    Ok(row.map(BackendModelRow::into_record))
}

pub async fn create_backend(
    id: &str,
    backend_type: &str,
    image: &str,
    use_for: Option<&HashSet<QuantType>>,
) -> Result<(), anyhow::Error> {
    let use_for_str = use_for.map(|u| serde_json::to_string(u).unwrap_or_default());

    sqlx::query(
        r"
        INSERT INTO backends (id, backend_type, image, use_for)
        VALUES (?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            backend_type = excluded.backend_type,
            image = excluded.image,
            use_for = excluded.use_for
        ",
    )
    .bind(id)
    .bind(backend_type)
    .bind(image)
    .bind(use_for_str)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn create_backend_model_config(
    backend_id: &str,
    model: &str,
    arguments: &[String],
    environment_variables: &HashMap<String, String>,
    ctx_length: Option<u64>,
) -> Result<(), anyhow::Error> {
    let args_str = serde_json::to_string(arguments)?;
    let env_str = serde_json::to_string(environment_variables)?;
    let ctx_i64 = ctx_length.map(u64::cast_signed);

    sqlx::query(
        r"
        INSERT INTO backend_model_configs (backend_id, model, arguments, environment_variables, ctx_length)
        VALUES (?, ?, ?, ?, ?)
        ON CONFLICT(backend_id, model) DO UPDATE SET
            arguments = excluded.arguments,
            environment_variables = excluded.environment_variables,
            ctx_length = excluded.ctx_length
        ",
    )
    .bind(backend_id)
    .bind(model)
    .bind(args_str)
    .bind(env_str)
    .bind(ctx_i64)
    .execute(get_executor())
    .await?;
    Ok(())
}
