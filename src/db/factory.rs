use crate::db::connection::get_executor;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunpodGpu {
    pub id: String,
    pub memory: u64,
    pub price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubernetesGpu {
    pub id: String,
    pub memory: u64,
    pub max_count: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactoryGpuRecord {
    pub factory_type: String,
    pub id: String,
    pub memory: u64,
    pub price: Option<f64>,
    pub max_count: Option<u64>,
    pub model: String,
}

#[derive(sqlx::FromRow)]
struct FactoryGpuRow {
    factory_type: Option<String>,
    id: String,
    memory: i64,
    price: Option<f64>,
    max_count: Option<i64>,
    model: Option<String>,
}

pub async fn get_all_factory_gpus(
    factory_type: Option<&str>,
    model: Option<&str>,
) -> Result<Vec<FactoryGpuRecord>, anyhow::Error> {
    let rows = match (factory_type, model) {
        (Some(ft), Some(m)) => {
            sqlx::query_as::<_, FactoryGpuRow>(
                r"
                SELECT factory_type, id, memory, price, max_count, model
                FROM factory_gpus
                WHERE factory_type = ? AND model = ?
                ORDER BY memory
                ",
            )
            .bind(ft)
            .bind(m)
            .fetch_all(get_executor())
            .await?
        }
        (Some(ft), None) => {
            sqlx::query_as::<_, FactoryGpuRow>(
                r"
                SELECT factory_type, id, memory, price, max_count, model
                FROM factory_gpus
                WHERE factory_type = ?
                ORDER BY memory
                ",
            )
            .bind(ft)
            .fetch_all(get_executor())
            .await?
        }
        (None, Some(m)) => {
            sqlx::query_as::<_, FactoryGpuRow>(
                r"
                SELECT factory_type, id, memory, price, max_count, model
                FROM factory_gpus
                WHERE model = ?
                ORDER BY memory
                ",
            )
            .bind(m)
            .fetch_all(get_executor())
            .await?
        }
        (None, None) => {
            sqlx::query_as::<_, FactoryGpuRow>(
                r"
                SELECT factory_type, id, memory, price, max_count, model
                FROM factory_gpus
                ORDER BY factory_type, memory
                ",
            )
            .fetch_all(get_executor())
            .await?
        }
    };

    Ok(rows
        .into_iter()
        .map(|r| FactoryGpuRecord {
            factory_type: r.factory_type.unwrap_or_default(),
            id: r.id,
            memory: r.memory as u64,
            price: r.price,
            max_count: r.max_count.map(|c| c as u64),
            model: r.model.unwrap_or_default(),
        })
        .collect())
}

pub async fn get_runpod_gpus(model: Option<&str>) -> Result<Vec<RunpodGpu>, anyhow::Error> {
    if let Some(m) = model {
        let model_rows = sqlx::query_as::<_, FactoryGpuRow>(
            r"
            SELECT factory_type, id, memory, price, max_count, model
            FROM factory_gpus
            WHERE factory_type = 'runpod' AND model = ?
            ORDER BY memory
            ",
        )
        .bind(m)
        .fetch_all(get_executor())
        .await?;

        if !model_rows.is_empty() {
            return Ok(model_rows
                .into_iter()
                .map(|r| RunpodGpu {
                    id: r.id,
                    memory: r.memory as u64,
                    price: r.price.unwrap_or(0.0),
                })
                .collect());
        }
    }

    let global_rows = sqlx::query_as::<_, FactoryGpuRow>(
        r"
        SELECT factory_type, id, memory, price, max_count, model
        FROM factory_gpus
        WHERE factory_type = 'runpod' AND model = ''
        ORDER BY memory
        ",
    )
    .fetch_all(get_executor())
    .await?;

    Ok(global_rows
        .into_iter()
        .map(|r| RunpodGpu {
            id: r.id,
            memory: r.memory as u64,
            price: r.price.unwrap_or(0.0),
        })
        .collect())
}

pub async fn get_kubernetes_gpus(model: Option<&str>) -> Result<Vec<KubernetesGpu>, anyhow::Error> {
    if let Some(m) = model {
        let model_rows = sqlx::query_as::<_, FactoryGpuRow>(
            r"
            SELECT factory_type, id, memory, price, max_count, model
            FROM factory_gpus
            WHERE factory_type = 'kubernetes' AND model = ?
            ORDER BY memory
            ",
        )
        .bind(m)
        .fetch_all(get_executor())
        .await?;

        if !model_rows.is_empty() {
            return Ok(model_rows
                .into_iter()
                .map(|r| KubernetesGpu {
                    id: r.id,
                    memory: r.memory as u64,
                    max_count: r.max_count.map(|c| c as u64),
                })
                .collect());
        }
    }

    let global_rows = sqlx::query_as::<_, FactoryGpuRow>(
        r"
        SELECT factory_type, id, memory, price, max_count, model
        FROM factory_gpus
        WHERE factory_type = 'kubernetes' AND model = ''
        ORDER BY memory
        ",
    )
    .fetch_all(get_executor())
    .await?;

    Ok(global_rows
        .into_iter()
        .map(|r| KubernetesGpu {
            id: r.id,
            memory: r.memory as u64,
            max_count: r.max_count.map(|c| c as u64),
        })
        .collect())
}

pub async fn create_factory_gpu(
    factory_type: &str,
    id: &str,
    memory: u64,
    price: Option<f64>,
    max_count: Option<u64>,
    model: Option<&str>,
) -> Result<(), anyhow::Error> {
    let memory_i64 = memory.cast_signed();
    let max_count_i64 = max_count.map(u64::cast_signed);
    let model_str = model.unwrap_or("");

    sqlx::query(
        r"
        INSERT INTO factory_gpus (factory_type, id, memory, price, max_count, model)
        VALUES (?, ?, ?, ?, ?, ?)
        ON CONFLICT(factory_type, id, model) DO UPDATE SET
            memory = excluded.memory,
            price = excluded.price,
            max_count = excluded.max_count
        ",
    )
    .bind(factory_type)
    .bind(id)
    .bind(memory_i64)
    .bind(price)
    .bind(max_count_i64)
    .bind(model_str)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn delete_factory_gpu(
    factory_type: &str,
    id: &str,
    model: Option<&str>,
) -> Result<bool, anyhow::Error> {
    let model_str = model.unwrap_or("");
    let res = sqlx::query(
        "DELETE FROM factory_gpus WHERE factory_type = ? AND id = ? AND model = ?",
    )
    .bind(factory_type)
    .bind(id)
    .bind(model_str)
    .execute(get_executor())
    .await?;
    Ok(res.rows_affected() > 0)
}
