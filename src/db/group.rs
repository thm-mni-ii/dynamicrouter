use crate::db::connection::get_executor;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub limit_to_models: Option<HashSet<String>>,
    pub created_at: i64,
}

#[derive(sqlx::FromRow)]
struct GroupRow {
    id: String,
    name: String,
    description: Option<String>,
    limit_to_models: Option<String>,
    created_at: i64,
}

impl GroupRow {
    fn into_group(self) -> Result<Group, serde_json::Error> {
        let limit_to_models = match self.limit_to_models {
            Some(s) if !s.trim().is_empty() => Some(serde_json::from_str(&s)?),
            _ => None,
        };
        Ok(Group {
            id: self.id,
            name: self.name,
            description: self.description,
            limit_to_models,
            created_at: self.created_at,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub limit_to_models: Option<HashSet<String>>,
    pub member_count: i64,
    pub rule_count: i64,
    pub rate_limit_count: i64,
    pub created_at: i64,
}

#[derive(sqlx::FromRow)]
struct GroupSummaryRow {
    id: String,
    name: String,
    description: Option<String>,
    limit_to_models: Option<String>,
    member_count: i64,
    rule_count: i64,
    rate_limit_count: i64,
    created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupPayload {
    pub name: String,
    pub description: Option<String>,
    pub limit_to_models: Option<HashSet<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGroupPayload {
    pub name: String,
    pub description: Option<String>,
    pub limit_to_models: Option<HashSet<String>>,
}

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize)]
pub struct GroupMember {
    pub id: String,
    pub group_id: String,
    pub username: String,
    pub is_manual: bool,
    pub joined_at: i64,
    pub left_at: Option<i64>,
    pub created_at: i64,
}

#[derive(sqlx::FromRow, Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GroupOidcRule {
    pub id: String,
    pub group_id: String,
    pub name: Option<String>,
    pub cel_expression: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateGroupOidcRulePayload {
    pub name: Option<String>,
    pub cel_expression: String,
}

pub async fn create_group(payload: &CreateGroupPayload) -> Result<Group, anyhow::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        anyhow::bail!("Group name cannot be empty");
    }

    let limit_str = payload
        .limit_to_models
        .as_ref()
        .map(|m| serde_json::to_string(m).unwrap_or_default());

    sqlx::query(
        r"
        INSERT INTO groups (id, name, description, limit_to_models, created_at)
        VALUES (?, ?, ?, ?, ?)
        ",
    )
    .bind(&id)
    .bind(&name)
    .bind(&payload.description)
    .bind(&limit_str)
    .bind(now)
    .execute(get_executor())
    .await?;

    Ok(Group {
        id,
        name,
        description: payload.description.clone(),
        limit_to_models: payload.limit_to_models.clone(),
        created_at: now,
    })
}

pub async fn update_group(
    id: &str,
    payload: &UpdateGroupPayload,
) -> Result<Option<Group>, anyhow::Error> {
    let name = payload.name.trim().to_string();
    if name.is_empty() {
        anyhow::bail!("Group name cannot be empty");
    }

    let limit_str = payload
        .limit_to_models
        .as_ref()
        .map(|m| serde_json::to_string(m).unwrap_or_default());

    let res = sqlx::query(
        r"
        UPDATE groups
        SET name = ?, description = ?, limit_to_models = ?
        WHERE id = ?
        ",
    )
    .bind(&name)
    .bind(&payload.description)
    .bind(&limit_str)
    .bind(id)
    .execute(get_executor())
    .await?;

    if res.rows_affected() == 0 {
        return Ok(None);
    }

    get_group_by_id(id).await
}

pub async fn delete_group(id: &str) -> Result<bool, anyhow::Error> {
    let res = sqlx::query("DELETE FROM groups WHERE id = ?")
        .bind(id)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}

pub async fn get_group_by_id(id: &str) -> Result<Option<Group>, anyhow::Error> {
    let row = sqlx::query_as::<_, GroupRow>(
        r"
        SELECT id, name, description, limit_to_models, created_at
        FROM groups
        WHERE id = ?
        ",
    )
    .bind(id)
    .fetch_optional(get_executor())
    .await?;

    match row {
        Some(r) => Ok(Some(r.into_group()?)),
        None => Ok(None),
    }
}

pub async fn get_group_by_name(name: &str) -> Result<Option<Group>, anyhow::Error> {
    let row = sqlx::query_as::<_, GroupRow>(
        r"
        SELECT id, name, description, limit_to_models, created_at
        FROM groups
        WHERE name = ?
        ",
    )
    .bind(name)
    .fetch_optional(get_executor())
    .await?;

    match row {
        Some(r) => Ok(Some(r.into_group()?)),
        None => Ok(None),
    }
}

pub async fn get_all_groups() -> Result<Vec<Group>, anyhow::Error> {
    let rows = sqlx::query_as::<_, GroupRow>(
        r"
        SELECT id, name, description, limit_to_models, created_at
        FROM groups
        ORDER BY name ASC
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let mut groups = Vec::with_capacity(rows.len());
    for row in rows {
        groups.push(row.into_group()?);
    }
    Ok(groups)
}

pub async fn get_groups_summary() -> Result<Vec<GroupSummary>, anyhow::Error> {
    let rows = sqlx::query_as::<_, GroupSummaryRow>(
        r"
        SELECT
            g.id,
            g.name,
            g.description,
            g.limit_to_models,
            g.created_at,
            (SELECT COUNT(DISTINCT username) FROM group_members gm WHERE gm.group_id = g.id AND gm.left_at IS NULL) AS member_count,
            (SELECT COUNT(*) FROM group_oidc_rules gor WHERE gor.group_id = g.id) AS rule_count,
            (SELECT COUNT(*) FROM rate_limits rl WHERE rl.group_name = g.name) AS rate_limit_count
        FROM groups g
        ORDER BY g.name ASC
        ",
    )
    .fetch_all(get_executor())
    .await?;

    let mut summaries = Vec::with_capacity(rows.len());
    for row in rows {
        let limit_to_models = match row.limit_to_models {
            Some(s) if !s.trim().is_empty() => Some(serde_json::from_str(&s)?),
            _ => None,
        };
        summaries.push(GroupSummary {
            id: row.id,
            name: row.name,
            description: row.description,
            limit_to_models,
            member_count: row.member_count,
            rule_count: row.rule_count,
            rate_limit_count: row.rate_limit_count,
            created_at: row.created_at,
        });
    }
    Ok(summaries)
}

pub async fn get_group_members(group_id: &str) -> Result<Vec<String>, anyhow::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        r"
        SELECT DISTINCT username
        FROM group_members
        WHERE group_id = ? AND left_at IS NULL
        ORDER BY username ASC
        ",
    )
    .bind(group_id)
    .fetch_all(get_executor())
    .await?;

    Ok(rows)
}

pub async fn add_group_member(group_id: &str, username: &str) -> Result<(), anyhow::Error> {
    let now = Utc::now().timestamp();
    let uname = username.trim();

    // Check if user is already an active member of this group
    let active = sqlx::query_as::<_, (String, bool)>(
        r"
        SELECT id, is_manual
        FROM group_members
        WHERE group_id = ? AND username = ? AND left_at IS NULL
        LIMIT 1
        ",
    )
    .bind(group_id)
    .bind(uname)
    .fetch_optional(get_executor())
    .await?;

    if let Some((id, is_manual)) = active {
        if !is_manual {
            sqlx::query("UPDATE group_members SET is_manual = 1 WHERE id = ?")
                .bind(id)
                .execute(get_executor())
                .await?;
        }
        return Ok(());
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query(
        r"
        INSERT INTO group_members (id, group_id, username, is_manual, joined_at, left_at, created_at)
        VALUES (?, ?, ?, 1, ?, NULL, ?)
        ",
    )
    .bind(&id)
    .bind(group_id)
    .bind(uname)
    .bind(now)
    .bind(now)
    .execute(get_executor())
    .await?;

    Ok(())
}

pub async fn remove_group_member(group_id: &str, username: &str) -> Result<bool, anyhow::Error> {
    let now = Utc::now().timestamp();
    let res = sqlx::query(
        r"
        UPDATE group_members
        SET left_at = ?
        WHERE group_id = ? AND username = ? AND left_at IS NULL
        ",
    )
    .bind(now)
    .bind(group_id)
    .bind(username.trim())
    .execute(get_executor())
    .await?;

    Ok(res.rows_affected() > 0)
}

pub async fn get_user_manual_groups(username: &str) -> Result<Vec<String>, anyhow::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        r"
        SELECT DISTINCT g.name
        FROM groups g
        JOIN group_members gm ON g.id = gm.group_id
        WHERE gm.username = ? AND gm.is_manual = 1 AND gm.left_at IS NULL
        ORDER BY g.name ASC
        ",
    )
    .bind(username.trim())
    .fetch_all(get_executor())
    .await?;

    Ok(rows)
}

pub async fn get_user_groups(username: &str) -> Result<Vec<String>, anyhow::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        r"
        SELECT DISTINCT g.name
        FROM groups g
        JOIN group_members gm ON g.id = gm.group_id
        WHERE gm.username = ? AND gm.left_at IS NULL
        ORDER BY g.name ASC
        ",
    )
    .bind(username.trim())
    .fetch_all(get_executor())
    .await?;

    Ok(rows)
}

pub async fn get_all_group_members_for_group_name(
    group_name: &str,
) -> Result<Vec<String>, anyhow::Error> {
    let rows = sqlx::query_scalar::<_, String>(
        r"
        SELECT DISTINCT gm.username
        FROM group_members gm
        JOIN groups g ON gm.group_id = g.id
        WHERE g.name = ? AND gm.left_at IS NULL
        ORDER BY gm.username ASC
        ",
    )
    .bind(group_name.trim())
    .fetch_all(get_executor())
    .await?;

    Ok(rows)
}

pub async fn sync_user_oidc_groups(
    username: &str,
    matched_group_names: &HashSet<String>,
) -> Result<(), anyhow::Error> {
    let now = Utc::now().timestamp();
    let uname = username.trim();

    #[derive(sqlx::FromRow)]
    struct ActiveMembership {
        id: String,
        group_name: String,
        is_manual: bool,
    }

    let active_rows = sqlx::query_as::<_, ActiveMembership>(
        r"
        SELECT gm.id, g.name AS group_name, gm.is_manual
        FROM group_members gm
        JOIN groups g ON gm.group_id = g.id
        WHERE gm.username = ? AND gm.left_at IS NULL
        ",
    )
    .bind(uname)
    .fetch_all(get_executor())
    .await?;

    // Mark memberships as left if dynamic and no longer matched
    for active in &active_rows {
        if !active.is_manual && !matched_group_names.contains(&active.group_name) {
            sqlx::query("UPDATE group_members SET left_at = ? WHERE id = ?")
                .bind(now)
                .bind(&active.id)
                .execute(get_executor())
                .await?;
        }
    }

    // Add new dynamic memberships
    for gname in matched_group_names {
        let is_already_active = active_rows.iter().any(|a| a.group_name == *gname);
        if !is_already_active {
            if let Some(group) = get_group_by_name(gname).await? {
                let id = Uuid::new_v4().to_string();
                sqlx::query(
                    r"
                    INSERT INTO group_members (id, group_id, username, is_manual, joined_at, left_at, created_at)
                    VALUES (?, ?, ?, 0, ?, NULL, ?)
                    ",
                )
                .bind(&id)
                .bind(&group.id)
                .bind(uname)
                .bind(now)
                .bind(now)
                .execute(get_executor())
                .await?;
            }
        }
    }

    Ok(())
}

pub async fn get_group_oidc_rules(group_id: &str) -> Result<Vec<GroupOidcRule>, anyhow::Error> {
    let rows = sqlx::query_as::<_, GroupOidcRule>(
        r"
        SELECT id, group_id, name, cel_expression, created_at
        FROM group_oidc_rules
        WHERE group_id = ?
        ORDER BY created_at ASC
        ",
    )
    .bind(group_id)
    .fetch_all(get_executor())
    .await?;

    Ok(rows)
}

pub async fn create_group_oidc_rule(
    group_id: &str,
    payload: &CreateGroupOidcRulePayload,
) -> Result<GroupOidcRule, anyhow::Error> {
    let id = Uuid::new_v4().to_string();
    let now = Utc::now().timestamp();
    let expr = payload.cel_expression.trim().to_string();
    if expr.is_empty() {
        anyhow::bail!("CEL expression cannot be empty");
    }

    sqlx::query(
        r"
        INSERT INTO group_oidc_rules (id, group_id, name, cel_expression, created_at)
        VALUES (?, ?, ?, ?, ?)
        ",
    )
    .bind(&id)
    .bind(group_id)
    .bind(&payload.name)
    .bind(&expr)
    .bind(now)
    .execute(get_executor())
    .await?;

    Ok(GroupOidcRule {
        id,
        group_id: group_id.to_string(),
        name: payload.name.clone(),
        cel_expression: expr,
        created_at: now,
    })
}

pub async fn update_group_oidc_rule(
    rule_id: &str,
    payload: &CreateGroupOidcRulePayload,
) -> Result<Option<GroupOidcRule>, anyhow::Error> {
    let expr = payload.cel_expression.trim().to_string();
    if expr.is_empty() {
        anyhow::bail!("CEL expression cannot be empty");
    }

    let res = sqlx::query(
        r"
        UPDATE group_oidc_rules
        SET name = ?, cel_expression = ?
        WHERE id = ?
        ",
    )
    .bind(&payload.name)
    .bind(&expr)
    .bind(rule_id)
    .execute(get_executor())
    .await?;

    if res.rows_affected() == 0 {
        return Ok(None);
    }

    let rule = sqlx::query_as::<_, GroupOidcRule>(
        r"
        SELECT id, group_id, name, cel_expression, created_at
        FROM group_oidc_rules
        WHERE id = ?
        ",
    )
    .bind(rule_id)
    .fetch_optional(get_executor())
    .await?;

    Ok(rule)
}

pub async fn delete_group_oidc_rule(rule_id: &str) -> Result<bool, anyhow::Error> {
    let res = sqlx::query("DELETE FROM group_oidc_rules WHERE id = ?")
        .bind(rule_id)
        .execute(get_executor())
        .await?;
    Ok(res.rows_affected() > 0)
}

#[derive(sqlx::FromRow)]
pub struct ActiveGroupOidcRuleRow {
    pub id: String,
    pub group_id: String,
    pub group_name: String,
    pub name: Option<String>,
    pub cel_expression: String,
    pub created_at: i64,
}

pub async fn get_all_active_group_oidc_rules() -> Result<Vec<ActiveGroupOidcRuleRow>, anyhow::Error>
{
    let rows = sqlx::query_as::<_, ActiveGroupOidcRuleRow>(
        r"
        SELECT gor.id, gor.group_id, g.name AS group_name, gor.name, gor.cel_expression, gor.created_at
        FROM group_oidc_rules gor
        JOIN groups g ON gor.group_id = g.id
        ORDER BY gor.created_at ASC
        ",
    )
    .fetch_all(get_executor())
    .await?;

    Ok(rows)
}

pub async fn get_allowed_models_for_groups(
    group_names: &[String],
) -> Result<HashSet<String>, anyhow::Error> {
    if group_names.is_empty() {
        return Ok(HashSet::new());
    }

    let mut allowed_models = HashSet::new();
    let all_groups = get_all_groups().await?;
    for group in all_groups {
        if group_names
            .iter()
            .any(|gn| gn.eq_ignore_ascii_case(&group.name))
        {
            if let Some(ref models) = group.limit_to_models {
                allowed_models.extend(models.clone());
            }
        }
    }

    Ok(allowed_models)
}
