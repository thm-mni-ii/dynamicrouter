use crate::db::connection::get_executor;

#[derive(sqlx::FromRow, Debug)]
pub struct Instance {
    pub id: String,
    pub model: String,
    pub data: Vec<u8>,
}

pub async fn get_instances_for(model: &str) -> Result<Vec<Instance>, anyhow::Error> {
    Ok(sqlx::query_as!(
        Instance,
        "SELECT id, model, data FROM instance WHERE model = ?",
        model
    )
    .fetch_all(get_executor())
    .await?)
}

pub async fn create_instance(instance: &Instance) -> Result<(), anyhow::Error> {
    sqlx::query!(
        "INSERT INTO instance (id, model, data) VALUES (?, ?, ?)",
        instance.id,
        instance.model,
        instance.data
    )
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn ensure_instance_exists(id: &str, model: &str) -> Result<(), anyhow::Error> {
    sqlx::query("INSERT OR IGNORE INTO instance (id, model, data) VALUES (?, ?, ?)")
        .bind(id)
        .bind(model)
        .bind(&b""[..])
        .execute(get_executor())
        .await?;
    Ok(())
}

pub async fn delete_instance(id: &str) -> Result<(), anyhow::Error> {
    sqlx::query!("DELETE FROM instance WHERE id = ?", id)
        .execute(get_executor())
        .await?;
    Ok(())
}
