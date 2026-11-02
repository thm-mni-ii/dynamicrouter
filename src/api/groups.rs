use crate::auth::AdminUser;
use crate::auth::cel::{OidcRuleContext, compile_cel, evaluate_cel};
use crate::db::group::{
    CreateGroupOidcRulePayload, CreateGroupPayload, UpdateGroupPayload, add_group_member,
    create_group, create_group_oidc_rule, delete_group, delete_group_oidc_rule,
    get_group_by_id, get_group_members, get_group_oidc_rules, get_groups_summary,
    remove_group_member, update_group, update_group_oidc_rule,
};
use crate::db::rate_limit::get_all_rate_limits;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub fn groups_routes() -> Router {
    Router::new()
        .route("/", get(list_groups).post(create_new_group))
        .route("/rules/test", post(test_cel_rule_endpoint))
        .route(
            "/{id}",
            get(get_single_group)
                .put(update_existing_group)
                .delete(remove_group),
        )
        .route(
            "/{id}/members",
            get(list_group_members_endpoint).post(add_member_endpoint),
        )
        .route("/{id}/members/{username}", delete(remove_member_endpoint))
        .route(
            "/{id}/rules",
            get(list_group_rules_endpoint).post(create_rule_endpoint),
        )
        .route(
            "/{id}/rules/{rule_id}",
            put(update_rule_endpoint).delete(delete_rule_endpoint),
        )
}

async fn list_groups(
    _admin: AdminUser,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let summaries = get_groups_summary().await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch groups: {e}") })),
        )
    })?;

    Ok(Json(json!({ "groups": summaries })))
}

async fn create_new_group(
    _admin: AdminUser,
    Json(payload): Json<CreateGroupPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let group = create_group(&payload).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to create group: {e}") })),
        )
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "group": group }))))
}

async fn get_single_group(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let group = get_group_by_id(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to fetch group: {e}") })),
        )
    })?;

    let Some(grp) = group else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Group not found" })),
        ));
    };

    let members = get_group_members(&id).await.unwrap_or_default();
    let rules = get_group_oidc_rules(&id).await.unwrap_or_default();
    let rate_limits = get_all_rate_limits(None, None, Some(&grp.name))
        .await
        .unwrap_or_default();

    Ok(Json(json!({
        "group": grp,
        "members": members,
        "rules": rules,
        "rate_limits": rate_limits,
    })))
}

async fn update_existing_group(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<UpdateGroupPayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let updated = update_group(&id, &payload).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to update group: {e}") })),
        )
    })?;

    match updated {
        Some(g) => Ok(Json(json!({ "group": g }))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Group not found" })),
        )),
    }
}

async fn remove_group(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_group(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete group: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted" })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Group not found" })),
        ))
    }
}

async fn list_group_members_endpoint(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let members = get_group_members(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list members: {e}") })),
        )
    })?;

    Ok(Json(json!({ "members": members })))
}

#[derive(Debug, Deserialize)]
pub struct AddMemberPayload {
    pub username: String,
}

async fn add_member_endpoint(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<AddMemberPayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    let username = payload.username.trim();
    if username.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Username cannot be empty" })),
        ));
    }

    add_group_member(&id, username).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to add member: {e}") })),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "status": "added", "username": username })),
    ))
}

async fn remove_member_endpoint(
    _admin: AdminUser,
    Path((id, username)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let removed = remove_group_member(&id, &username).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to remove member: {e}") })),
        )
    })?;

    if removed {
        Ok(Json(json!({ "status": "removed", "username": username })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Member not found in group" })),
        ))
    }
}

async fn list_group_rules_endpoint(
    _admin: AdminUser,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let rules = get_group_oidc_rules(&id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to list rules: {e}") })),
        )
    })?;

    Ok(Json(json!({ "rules": rules })))
}

async fn create_rule_endpoint(
    _admin: AdminUser,
    Path(id): Path<String>,
    Json(payload): Json<CreateGroupOidcRulePayload>,
) -> Result<(StatusCode, Json<serde_json::Value>), (StatusCode, Json<serde_json::Value>)> {
    // Validate CEL syntax upfront
    compile_cel(&payload.cel_expression).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("CEL syntax error: {e}") })),
        )
    })?;

    let rule = create_group_oidc_rule(&id, &payload).await.map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("Failed to create rule: {e}") })),
        )
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "rule": rule }))))
}

async fn update_rule_endpoint(
    _admin: AdminUser,
    Path((_id, rule_id)): Path<(String, String)>,
    Json(payload): Json<CreateGroupOidcRulePayload>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    // Validate CEL syntax upfront
    compile_cel(&payload.cel_expression).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("CEL syntax error: {e}") })),
        )
    })?;

    let updated = update_group_oidc_rule(&rule_id, &payload)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": format!("Failed to update rule: {e}") })),
            )
        })?;

    match updated {
        Some(r) => Ok(Json(json!({ "rule": r }))),
        None => Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Rule not found" })),
        )),
    }
}

async fn delete_rule_endpoint(
    _admin: AdminUser,
    Path((_id, rule_id)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, Json<serde_json::Value>)> {
    let deleted = delete_group_oidc_rule(&rule_id).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": format!("Failed to delete rule: {e}") })),
        )
    })?;

    if deleted {
        Ok(Json(json!({ "status": "deleted" })))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Rule not found" })),
        ))
    }
}

#[derive(Debug, Deserialize)]
pub struct TestCelRequest {
    pub cel_expression: String,
    pub sample_claims: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct TestCelResponse {
    pub valid: bool,
    pub matched: bool,
    pub error: Option<String>,
}

async fn test_cel_rule_endpoint(
    _admin: AdminUser,
    Json(payload): Json<TestCelRequest>,
) -> Json<TestCelResponse> {
    let program = match compile_cel(&payload.cel_expression) {
        Ok(p) => p,
        Err(e) => {
            return Json(TestCelResponse {
                valid: false,
                matched: false,
                error: Some(format!("Syntax error: {e}")),
            });
        }
    };

    let sample_val = payload
        .sample_claims
        .unwrap_or_else(|| serde_json::json!({}));
    let u_claims = crate::auth::oidc::extract_claims_from_json(&sample_val);

    let rule_ctx = OidcRuleContext {
        sub: u_claims.sub,
        name: u_claims.name,
        preferred_username: u_claims.preferred_username,
        email: u_claims.email,
        roles: u_claims.roles.unwrap_or_default(),
        groups: u_claims.groups.unwrap_or_default(),
        claims: sample_val,
    };

    match evaluate_cel(&program, &rule_ctx) {
        Ok(matched) => Json(TestCelResponse {
            valid: true,
            matched,
            error: None,
        }),
        Err(e) => Json(TestCelResponse {
            valid: true,
            matched: false,
            error: Some(format!("Evaluation error: {e}")),
        }),
    }
}
