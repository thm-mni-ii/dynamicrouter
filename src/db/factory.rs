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

#[derive(sqlx::FromRow)]
struct FactoryGpuRow {
    id: String,
    memory: i64,
    price: Option<f64>,
    max_count: Option<i64>,
}

pub async fn get_runpod_gpus(model: Option<&str>) -> Result<Vec<RunpodGpu>, anyhow::Error> {
    if let Some(m) = model {
        let model_rows = sqlx::query_as::<_, FactoryGpuRow>(
            r"
            SELECT id, memory, price, max_count
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
        SELECT id, memory, price, max_count
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
            SELECT id, memory, price, max_count
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
        SELECT id, memory, price, max_count
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
