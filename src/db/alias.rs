use crate::db::connection::get_executor;
use std::collections::HashMap;

#[derive(sqlx::FromRow)]
struct AliasRow {
    alias: String,
    target: String,
}

pub async fn resolve_alias(alias: &str) -> Result<Option<String>, anyhow::Error> {
    let row = sqlx::query_scalar::<_, String>("SELECT target FROM aliases WHERE alias = ?")
        .bind(alias)
        .fetch_optional(get_executor())
        .await?;

    Ok(row)
}

pub async fn get_alias(alias: &str) -> Result<Option<String>, anyhow::Error> {
    resolve_alias(alias).await
}

pub async fn get_all_aliases() -> Result<HashMap<String, String>, anyhow::Error> {
    let rows = sqlx::query_as::<_, AliasRow>("SELECT alias, target FROM aliases ORDER BY alias")
        .fetch_all(get_executor())
        .await?;

    let mut map = HashMap::new();
    for row in rows {
        map.insert(row.alias, row.target);
    }
    Ok(map)
}

pub async fn create_alias(alias: &str, target: &str) -> Result<(), anyhow::Error> {
    sqlx::query(
        r"
        INSERT INTO aliases (alias, target)
        VALUES (?, ?)
        ON CONFLICT(alias) DO UPDATE SET target = excluded.target
        ",
    )
    .bind(alias)
    .bind(target)
    .execute(get_executor())
    .await?;
    Ok(())
}

pub async fn delete_alias(alias: &str) -> Result<bool, anyhow::Error> {
    let res = sqlx::query("DELETE FROM aliases WHERE alias = ?")
        .bind(alias)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}
