use crate::db::connection::get_executor;
use uuid::Uuid;

#[derive(sqlx::FromRow, Debug)]
#[allow(unused, clippy::struct_field_names)]
pub struct Request {
    pub id: String,
    pub model: String,
    pub enqueue: i64,
    pub done: Option<i64>,
    pub instance: String,
    pub user: Option<String>,
    pub request_body: Option<String>,
    pub response_body: Option<String>,
}

pub async fn count_pending_requests_for(model: &str) -> Result<i64, anyhow::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(DISTINCT id) FROM request WHERE model = ? AND done IS NULL",
    )
    .bind(model)
    .fetch_one(get_executor())
    .await?;
    Ok(count)
}

pub async fn latest_for_each_model() -> Result<Vec<Request>, anyhow::Error> {
    Ok(sqlx::query_as::<_, Request>(
        "SELECT id, model, MAX(enqueue) AS enqueue, done, instance, user, request_body, response_body FROM request GROUP BY model"
    )
    .fetch_all(get_executor())
    .await?)
}

pub async fn create_request(
    model: &str,
    instance: &str,
    user: &str,
    request_body: Option<&serde_json::Value>,
) -> Result<String, anyhow::Error> {
    let id = Uuid::new_v4().to_string();
    let body_str = request_body.map(serde_json::to_string).transpose()?;
    sqlx::query(
        "INSERT INTO request (id, model, enqueue, instance, user, request_body) VALUES (?, ?, unixepoch(), ?, ?, ?)",
    )
    .bind(&id)
    .bind(model)
    .bind(instance)
    .bind(user)
    .bind(body_str)
    .execute(get_executor())
    .await?;
    Ok(id)
}

pub async fn request_done(id: &str) -> Result<(), anyhow::Error> {
    sqlx::query("UPDATE request SET done = unixepoch() WHERE id = ?")
        .bind(id)
        .execute(get_executor())
        .await?;
    Ok(())
}

pub async fn save_response_body(id: &str, body: &str) -> Result<(), anyhow::Error> {
    sqlx::query("UPDATE request SET response_body = ? WHERE id = ?")
        .bind(body)
        .bind(id)
        .execute(get_executor())
        .await?;
    Ok(())
}
