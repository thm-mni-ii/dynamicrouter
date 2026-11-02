use crate::api::{
    add_admin_group_member, create_admin_group, create_admin_group_rule,
    delete_admin_group, delete_admin_group_rule, delete_admin_rate_limit,
    fetch_admin_group_detail, fetch_admin_groups, fetch_v1_models,
    remove_admin_group_member, save_admin_rate_limit, test_cel_rule, update_admin_group,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::{
    CelTestRequest, CreateGroupOidcRulePayload, CreateGroupPayload, GroupDetail,
    GroupSummary, RateLimitRecord, UpdateGroupPayload,
};
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DetailTab {
    Members,
    Rules,
    RateLimits,
}

fn format_count(val: i64) -> String {
    if val >= 1_000_000_000 {
        format!("{:.1}B", val as f64 / 1_000_000_000.0)
    } else if val >= 1_000_000 {
        format!("{:.1}M", val as f64 / 1_000_000.0)
    } else if val >= 1_000 {
        format!("{:.1}K", val as f64 / 1_000.0)
    } else {
        val.to_string()
    }
}

fn load_groups_action(
    mut is_loading: Signal<bool>,
    mut groups: Signal<Vec<GroupSummary>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_groups().await {
            Ok(list) => groups.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load groups: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

fn reload_group_detail_action(
    mut detail_data: Signal<Option<GroupDetail>>,
    mut toasts: Signal<Vec<Toast>>,
    id: String,
) {
    spawn(async move {
        match fetch_admin_group_detail(&id).await {
            Ok(data) => detail_data.set(Some(data)),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load group details: {e}"),
                    is_error: true,
                });
            }
        }
    });
}

#[component]
pub fn AdminGroupsView(toasts: Signal<Vec<Toast>>) -> Element {
    let groups = use_signal(Vec::<GroupSummary>::new);
    let mut model_suggestions = use_signal(Vec::<String>::new);
    let is_loading = use_signal(|| false);

    // Modal state for Group Create/Edit
    let mut editing_group = use_signal(|| Option::<(CreateGroupPayload, Option<String>)>::None);
    let mut form_name = use_signal(String::new);
    let mut form_desc = use_signal(String::new);
    let mut form_models = use_signal(Vec::<String>::new);

    // Detailed manage modal
    let mut active_detail_id = use_signal(|| Option::<String>::None);
    let mut detail_data = use_signal(|| Option::<GroupDetail>::None);
    let mut active_detail_tab = use_signal(|| DetailTab::Members);
    let mut new_member_username = use_signal(String::new);

    // OIDC Rule creation state
    let mut rule_name = use_signal(String::new);
    let mut rule_cel = use_signal(String::new);
    let mut test_sample_json = use_signal(|| {
        serde_json::json!({
            "email": "researcher@university.edu",
            "roles": ["researcher", "ai-team"],
            "groups": ["engineering"],
            "claims": {
                "department": "AI-Lab"
            }
        })
        .to_string()
    });
    let mut test_result = use_signal(|| Option::<(bool, bool, Option<String>)>::None);

    // Visual builder helpers
    let mut visual_claim = use_signal(|| "groups".to_string());
    let mut visual_op = use_signal(|| "contains".to_string());
    let mut visual_val = use_signal(|| "engineering".to_string());

    // Group Rate Limit creation state
    let mut rl_model = use_signal(String::new);
    let mut rl_scope_mode = use_signal(|| "per_user".to_string());
    let mut rl_time_window = use_signal(|| "minute".to_string());
    let mut rl_limit_str = use_signal(|| "100000".to_string());
    let mut rl_unit = use_signal(|| "tokens".to_string());

    let _ = use_resource(move || async move {
        load_groups_action(is_loading, groups, toasts);
        if let Ok(models) = fetch_v1_models().await {
            let mut list: Vec<String> = models.into_iter().map(|m| m.id).collect();
            list.sort();
            model_suggestions.set(list);
        }
    });

    let open_create_modal = move |_| {
        form_name.set(String::new());
        form_desc.set(String::new());
        form_models.set(Vec::new());
        editing_group.set(Some((
            CreateGroupPayload {
                name: String::new(),
                description: None,
                limit_to_models: None,
            },
            None,
        )));
    };

    let open_edit_modal = move |grp: GroupSummary| {
        let mut form_name = form_name;
        let mut form_desc = form_desc;
        let mut form_models = form_models;
        let mut editing_group = editing_group;
        form_name.set(grp.name.clone());
        form_desc.set(grp.description.clone().unwrap_or_default());
        let models_vec = grp
            .limit_to_models
            .clone()
            .map(|s| s.into_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        form_models.set(models_vec);
        editing_group.set(Some((
            CreateGroupPayload {
                name: grp.name,
                description: grp.description,
                limit_to_models: grp.limit_to_models,
            },
            Some(grp.id),
        )));
    };

    let save_group = move |_| {
        let (_payload, existing_id) = match editing_group.read().clone() {
            Some(p) => p,
            None => return,
        };

        let name = form_name.read().trim().to_string();
        if name.is_empty() {
            toasts.write().push(Toast {
                id: js_sys::Date::now() as u64,
                message: "Group name cannot be empty".to_string(),
                is_error: true,
            });
            return;
        }

        let desc_trimmed = form_desc.read().trim().to_string();
        let description = if desc_trimmed.is_empty() {
            None
        } else {
            Some(desc_trimmed)
        };

        let models_vec = form_models.read().clone();
        let limit_to_models = if models_vec.is_empty() {
            None
        } else {
            Some(models_vec.into_iter().collect::<HashSet<_>>())
        };

        spawn(async move {
            if let Some(id) = existing_id {
                let update_payload = UpdateGroupPayload {
                    name,
                    description,
                    limit_to_models,
                };
                match update_admin_group(&id, &update_payload).await {
                    Ok(_) => {
                        toasts.write().push(Toast {
                            id: js_sys::Date::now() as u64,
                            message: "Group updated successfully.".to_string(),
                            is_error: false,
                        });
                        editing_group.set(None);
                        load_groups_action(is_loading, groups, toasts);
                    }
                    Err(e) => {
                        toasts.write().push(Toast {
                            id: js_sys::Date::now() as u64,
                            message: format!("Error updating group: {e}"),
                            is_error: true,
                        });
                    }
                }
            } else {
                let create_payload = CreateGroupPayload {
                    name,
                    description,
                    limit_to_models,
                };
                match create_admin_group(&create_payload).await {
                    Ok(_) => {
                        toasts.write().push(Toast {
                            id: js_sys::Date::now() as u64,
                            message: "Group created successfully.".to_string(),
                            is_error: false,
                        });
                        editing_group.set(None);
                        load_groups_action(is_loading, groups, toasts);
                    }
                    Err(e) => {
                        toasts.write().push(Toast {
                            id: js_sys::Date::now() as u64,
                            message: format!("Error creating group: {e}"),
                            is_error: true,
                        });
                    }
                }
            }
        });
    };

    let delete_group_action = move |id: String, name: String| {
        let confirmed = web_sys::window()
            .and_then(|w| w.confirm_with_message(&format!("Delete group '{name}' and remove its assignments?")).ok())
            .unwrap_or(false);
        if !confirmed {
            return;
        }

        spawn(async move {
            match delete_admin_group(&id).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Group deleted successfully.".to_string(),
                        is_error: false,
                    });
                    load_groups_action(is_loading, groups, toasts);
                }
                Err(e) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to delete group: {e}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let open_manage_detail = move |id: String| {
        let mut active_detail_id = active_detail_id;
        let mut active_detail_tab = active_detail_tab;
        active_detail_id.set(Some(id.clone()));
        active_detail_tab.set(DetailTab::Members);
        reload_group_detail_action(detail_data, toasts, id);
    };

    let generate_visual_cel = move |_| {
        let claim = visual_claim.read().clone();
        let op = visual_op.read().clone();
        let val = visual_val.read().clone();

        let generated = match (claim.as_str(), op.as_str()) {
            ("email", "ends_with") => format!("email.endsWith('{val}')"),
            ("email", "equals") => format!("email == '{val}'"),
            ("groups", "contains") => format!("'{val}' in groups"),
            ("roles", "contains") => format!("'{val}' in roles"),
            ("preferred_username", "equals") => format!("preferred_username == '{val}'"),
            (custom, "equals") => format!("has(claims.{custom}) && claims.{custom} == '{val}'"),
            (custom, "contains") => format!("'{val}' in claims.{custom}"),
            _ => format!("'{val}' in {claim}"),
        };
        rule_cel.set(generated);
    };

    let test_cel_action = move |_| {
        let expr = rule_cel.read().trim().to_string();
        if expr.is_empty() {
            return;
        }

        let sample_parsed: Option<serde_json::Value> =
            serde_json::from_str(&test_sample_json.read()).ok();

        spawn(async move {
            match test_cel_rule(&CelTestRequest {
                cel_expression: expr,
                sample_claims: sample_parsed,
            })
            .await
            {
                Ok(res) => {
                    test_result.set(Some((res.valid, res.matched, res.error)));
                }
                Err(e) => {
                    test_result.set(Some((false, false, Some(e))));
                }
            }
        });
    };

    let add_member_action = move |_| {
        let gid = match active_detail_id.read().clone() {
            Some(id) => id,
            None => return,
        };
        let uname = new_member_username.read().trim().to_string();
        if uname.is_empty() {
            return;
        }

        spawn(async move {
            match add_admin_group_member(&gid, &uname).await {
                Ok(_) => {
                    new_member_username.set(String::new());
                    reload_group_detail_action(detail_data, toasts, gid);
                    load_groups_action(is_loading, groups, toasts);
                }
                Err(e) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to add member: {e}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let remove_member_action = move |uname: String| {
        let gid = match active_detail_id.read().clone() {
            Some(id) => id,
            None => return,
        };

        spawn(async move {
            match remove_admin_group_member(&gid, &uname).await {
                Ok(_) => {
                    reload_group_detail_action(detail_data, toasts, gid);
                    load_groups_action(is_loading, groups, toasts);
                }
                Err(e) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to remove member: {e}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let save_rule_action = move |_| {
        let gid = match active_detail_id.read().clone() {
            Some(id) => id,
            None => return,
        };
        let expr = rule_cel.read().trim().to_string();
        if expr.is_empty() {
            toasts.write().push(Toast {
                id: js_sys::Date::now() as u64,
                message: "CEL expression cannot be empty".to_string(),
                is_error: true,
            });
            return;
        }

        let name_str = rule_name.read().trim().to_string();
        let name_opt = if name_str.is_empty() {
            None
        } else {
            Some(name_str)
        };

        spawn(async move {
            let payload = CreateGroupOidcRulePayload {
                name: name_opt,
                cel_expression: expr,
            };
            match create_admin_group_rule(&gid, &payload).await {
                Ok(_) => {
                    rule_name.set(String::new());
                    rule_cel.set(String::new());
                    test_result.set(None);
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "OIDC CEL rule created successfully.".to_string(),
                        is_error: false,
                    });
                    reload_group_detail_action(detail_data, toasts, gid);
                    load_groups_action(is_loading, groups, toasts);
                }
                Err(e) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to save rule: {e}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let delete_rule_action = move |rule_id: String| {
        let gid = match active_detail_id.read().clone() {
            Some(id) => id,
            None => return,
        };

        spawn(async move {
            match delete_admin_group_rule(&gid, &rule_id).await {
                Ok(_) => {
                    reload_group_detail_action(detail_data, toasts, gid);
                    load_groups_action(is_loading, groups, toasts);
                }
                Err(e) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to delete rule: {e}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let save_group_rate_limit = move |_| {
        let detail = match detail_data.read().clone() {
            Some(d) => d,
            None => return,
        };

        let limit_val = rl_limit_str.read().trim().parse::<i64>().unwrap_or(0);
        if limit_val <= 0 {
            toasts.write().push(Toast {
                id: js_sys::Date::now() as u64,
                message: "Please enter a valid rate limit value greater than 0".to_string(),
                is_error: true,
            });
            return;
        }

        let model_val = rl_model.read().trim().to_string();
        let model_opt = if model_val.is_empty() || model_val == "*" {
            None
        } else {
            Some(model_val)
        };

        let record = RateLimitRecord {
            id: String::new(),
            user: None,
            token_hash: None,
            group_name: Some(detail.group.name.clone()),
            scope_mode: rl_scope_mode.read().clone(),
            model: model_opt,
            time_window: rl_time_window.read().clone(),
            limit_value: limit_val,
            unit: rl_unit.read().clone(),
            created_at: 0,
        };

        let gid = detail.group.id.clone();
        spawn(async move {
            match save_admin_rate_limit(&record, true).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Group rate limit added successfully.".to_string(),
                        is_error: false,
                    });
                    reload_group_detail_action(detail_data, toasts, gid);
                    load_groups_action(is_loading, groups, toasts);
                }
                Err(e) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to create rate limit: {e}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let delete_rate_limit_action = move |rl_id: String| {
        let gid = match active_detail_id.read().clone() {
            Some(id) => id,
            None => return,
        };

        spawn(async move {
            match delete_admin_rate_limit(&rl_id).await {
                Ok(_) => {
                    reload_group_detail_action(detail_data, toasts, gid);
                    load_groups_action(is_loading, groups, toasts);
                }
                Err(e) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to delete rate limit: {e}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let groups_list = groups.read().clone();

    rsx! {
        div {
            class: "flex flex-col gap-4",
            
            GlassCard {
                div {
                    class: "card-header",
                    div {
                        h2 {
                            class: "card-title",
                            IconUsers { size: 20 }
                            span { "Group Administration" }
                        }
                        p { class: "card-description", "Organize users into groups to grant model access permissions and enforce shared or per-member rate limits." }
                    }
                    div {
                        class: "flex items-center gap-3",
                        button {
                            class: "glass-btn btn-primary",
                            onclick: open_create_modal,
                            IconPlus { size: 16 }
                            span { "Add Group" }
                        }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Group Name" }
                                th { "Description" }
                                th { "Granted Models" }
                                th { "Members" }
                                th { "OIDC Rules" }
                                th { "Rate Limits" }
                                th { class: "text-right whitespace-nowrap", "Actions" }
                            }
                        }
                        tbody {
                            if groups_list.is_empty() {
                                tr {
                                    td {
                                        colspan: 7,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() {
                                            "Loading groups..."
                                        } else {
                                            "No groups configured yet. Click 'Add Group' to create one."
                                        }
                                    }
                                }
                            } else {
                                for grp in groups_list {
                                    {
                                        let grp_clone = grp.clone();
                                        let grp_id = grp.id.clone();
                                        let grp_name = grp.name.clone();
                                        rsx! {
                                            tr {
                                                key: "{grp.id}",
                                                td {
                                                    class: "font-semibold",
                                                    span {
                                                        class: "cursor-pointer text-primary hover:underline",
                                                        onclick: move |_| open_manage_detail(grp_id.clone()),
                                                        "{grp.name}"
                                                    }
                                                }
                                                td {
                                                    class: "text-muted text-sm",
                                                    "{grp.description.as_deref().unwrap_or(\"-\")}"
                                                }
                                                td {
                                                    div {
                                                        class: "flex flex-wrap gap-1",
                                                        if let Some(ref models) = grp.limit_to_models {
                                                            for m in models {
                                                                GlassBadge { "{m}" }
                                                            }
                                                        } else {
                                                            span { class: "text-muted text-xs", "No group restrictions" }
                                                        }
                                                    }
                                                }
                                                td {
                                                    GlassBadge { "{grp.member_count} members" }
                                                }
                                                td {
                                                    GlassBadge { "{grp.rule_count} rules" }
                                                }
                                                td {
                                                    GlassBadge { "{grp.rate_limit_count} limits" }
                                                }
                                                td {
                                                    class: "text-right whitespace-nowrap",
                                                    div {
                                                        class: "flex justify-end gap-2",
                                                        button {
                                                            class: "glass-btn btn-sm btn-secondary",
                                                            onclick: {
                                                                let gid = grp.id.clone();
                                                                move |_| open_manage_detail(gid.clone())
                                                            },
                                                            IconSliders { size: 14 }
                                                            span { "Manage" }
                                                        }
                                                        button {
                                                            class: "glass-btn btn-sm btn-secondary",
                                                            onclick: move |_| open_edit_modal(grp_clone.clone()),
                                                            span { "Edit" }
                                                        }
                                                        button {
                                                            class: "glass-btn btn-sm btn-danger",
                                                            onclick: move |_| delete_group_action(grp.id.clone(), grp_name.clone()),
                                                            IconTrash { size: 14 }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Create/Edit Group Modal
            if editing_group.read().is_some() {
                {
                    let is_new = editing_group.read().as_ref().unwrap().1.is_none();
                    let title = if is_new { "Create New Group" } else { "Edit Group" };
                    rsx! {
                        GlassModal {
                            title: title.to_string(),
                            on_close: move |_| editing_group.set(None),
                            div {
                                class: "flex flex-col gap-4",
                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Group Identifier / Name" }
                                    input {
                                        r#type: "text",
                                        class: "glass-input",
                                        placeholder: "e.g. engineering, ai-team, researchers",
                                        value: "{form_name.read()}",
                                        oninput: move |evt| form_name.set(evt.value()),
                                    }
                                }
                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Description (Optional)" }
                                    input {
                                        r#type: "text",
                                        class: "glass-input",
                                        placeholder: "Brief note on group role and access...",
                                        value: "{form_desc.read()}",
                                        oninput: move |evt| form_desc.set(evt.value()),
                                    }
                                }
                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Granted Models (Additive Whitelist)" }
                                    p { class: "form-hint mb-2", "Members of this group gain access to these models in addition to their personal whitelists. Supports glob patterns (e.g. meta-llama/*)." }
                                    ChipInput {
                                        values: form_models,
                                        suggestions: model_suggestions.read().clone(),
                                        placeholder: "Add model ID or glob pattern...",
                                    }
                                }
                                div {
                                    class: "modal-footer mt-4",
                                    button {
                                        class: "glass-btn btn-secondary",
                                        onclick: move |_| editing_group.set(None),
                                        "Cancel"
                                    }
                                    button {
                                        class: "glass-btn btn-primary",
                                        onclick: save_group,
                                        if is_new { "Create Group" } else { "Save Changes" }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Group Detail & Management Modal
            if active_detail_id.read().is_some() {
                {
                    let detail_opt = detail_data.read().clone();
                    rsx! {
                        GlassModal {
                            title: if let Some(ref d) = detail_opt {
                                format!("Manage Group: {}", d.group.name)
                            } else {
                                "Manage Group".to_string()
                            },
                            on_close: move |_| {
                                active_detail_id.set(None);
                                detail_data.set(None);
                            },
                            if let Some(detail) = detail_opt {
                                div {
                                    class: "flex flex-col gap-4",
                                    // Tabs navigation
                                    div {
                                        class: "flex items-center gap-2 border-b border-white/10 pb-2 mb-2",
                                        button {
                                            class: if *active_detail_tab.read() == DetailTab::Members { "glass-btn btn-sm btn-primary" } else { "glass-btn btn-sm btn-secondary" },
                                            onclick: move |_| active_detail_tab.set(DetailTab::Members),
                                            IconUsers { size: 14 }
                                            span { "Members ({detail.members.len()})" }
                                        }
                                        button {
                                            class: if *active_detail_tab.read() == DetailTab::Rules { "glass-btn btn-sm btn-primary" } else { "glass-btn btn-sm btn-secondary" },
                                            onclick: move |_| active_detail_tab.set(DetailTab::Rules),
                                            IconBolt { size: 14 }
                                            span { "OIDC CEL Rules ({detail.rules.len()})" }
                                        }
                                        button {
                                            class: if *active_detail_tab.read() == DetailTab::RateLimits { "glass-btn btn-sm btn-primary" } else { "glass-btn btn-sm btn-secondary" },
                                            onclick: move |_| active_detail_tab.set(DetailTab::RateLimits),
                                            IconGauge { size: 14 }
                                            span { "Group Rate Limits ({detail.rate_limits.len()})" }
                                        }
                                    }

                                    // Tab 1: Members Management
                                    if *active_detail_tab.read() == DetailTab::Members {
                                        div {
                                            class: "flex flex-col gap-3",
                                            div {
                                                class: "flex items-center gap-2",
                                                input {
                                                    r#type: "text",
                                                    class: "glass-input flex-1",
                                                    placeholder: "Enter existing username to add to group...",
                                                    value: "{new_member_username.read()}",
                                                    oninput: move |evt| new_member_username.set(evt.value()),
                                                    onkeydown: move |evt: KeyboardEvent| {
                                                        if evt.key() == Key::Enter {
                                                            add_member_action(());
                                                        }
                                                    }
                                                }
                                                button {
                                                    class: "glass-btn btn-primary",
                                                    onclick: move |_| add_member_action(()),
                                                    IconPlus { size: 14 }
                                                    span { "Add Member" }
                                                }
                                            }

                                            div {
                                                class: "glass-table-container max-h-60 overflow-y-auto mt-2",
                                                table {
                                                    class: "glass-table",
                                                    thead {
                                                        tr {
                                                            th { "Username" }
                                                            th { class: "text-right", "Action" }
                                                        }
                                                    }
                                                    tbody {
                                                        if detail.members.is_empty() {
                                                            tr {
                                                                td {
                                                                    colspan: 2,
                                                                    class: "text-muted text-center py-4",
                                                                    "No manual members assigned to this group yet."
                                                                }
                                                            }
                                                        } else {
                                                            for member in detail.members {
                                                                {
                                                                    let m_uname = member.clone();
                                                                    rsx! {
                                                                        tr {
                                                                            key: "{member}",
                                                                            td { class: "font-mono text-sm", "{member}" }
                                                                            td {
                                                                                class: "text-right",
                                                                                button {
                                                                                    class: "glass-btn btn-sm btn-danger",
                                                                                    onclick: move |_| remove_member_action(m_uname.clone()),
                                                                                    IconTrash { size: 12 }
                                                                                    span { "Remove" }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Tab 2: OIDC Rules (CEL) Management
                                    if *active_detail_tab.read() == DetailTab::Rules {
                                        div {
                                            class: "flex flex-col gap-4",
                                            // Existing rules list
                                            div {
                                                class: "glass-table-container max-h-48 overflow-y-auto",
                                                table {
                                                    class: "glass-table",
                                                    thead {
                                                        tr {
                                                            th { "Rule Name" }
                                                            th { "CEL Expression" }
                                                            th { class: "text-right", "Action" }
                                                        }
                                                    }
                                                    tbody {
                                                        if detail.rules.is_empty() {
                                                            tr {
                                                                td {
                                                                    colspan: 3,
                                                                    class: "text-muted text-center py-4",
                                                                    "No OIDC profile rules yet. Users matching these rules are dynamically granted this group on login."
                                                                }
                                                            }
                                                        } else {
                                                            for r in detail.rules {
                                                                {
                                                                    let rid = r.id.clone();
                                                                    rsx! {
                                                                        tr {
                                                                            key: "{r.id}",
                                                                            td { class: "font-semibold text-sm", "{r.name.as_deref().unwrap_or(\"Untitled Rule\")}" }
                                                                            td { class: "font-mono text-xs text-primary", "{r.cel_expression}" }
                                                                            td {
                                                                                class: "text-right",
                                                                                button {
                                                                                    class: "glass-btn btn-sm btn-danger",
                                                                                    onclick: move |_| delete_rule_action(rid.clone()),
                                                                                    IconTrash { size: 12 }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Rule Authoring Panel
                                            div {
                                                class: "p-4 rounded-xl border border-white/10 flex flex-col gap-3",
                                                style: "background: var(--glow-surface);",
                                                h4 { class: "text-sm font-bold flex items-center gap-2",
                                                    IconPlus { size: 14 }
                                                    span { "Add Profile Matching Rule (Common Expression Language)" }
                                                }

                                                // Visual Expression Builder
                                                div {
                                                    class: "flex flex-wrap items-center gap-2 p-2 rounded-lg bg-black/20 border border-white/5",
                                                    span { class: "text-xs text-muted font-semibold", "Expression Helper:" }
                                                    select {
                                                        class: "glass-select text-xs py-1 px-2",
                                                        value: "{visual_claim.read()}",
                                                        onchange: move |evt| visual_claim.set(evt.value()),
                                                        option { value: "groups", "Claim: groups" }
                                                        option { value: "roles", "Claim: roles" }
                                                        option { value: "email", "Claim: email" }
                                                        option { value: "preferred_username", "Claim: preferred_username" }
                                                        option { value: "department", "Custom: claims.department" }
                                                    }
                                                    select {
                                                        class: "glass-select text-xs py-1 px-2",
                                                        value: "{visual_op.read()}",
                                                        onchange: move |evt| visual_op.set(evt.value()),
                                                        option { value: "contains", "contains" }
                                                        option { value: "equals", "equals (==)" }
                                                        option { value: "ends_with", "ends with (.endsWith)" }
                                                    }
                                                    input {
                                                        r#type: "text",
                                                        class: "glass-input text-xs py-1 px-2 flex-1 min-w-[120px]",
                                                        placeholder: "value...",
                                                        value: "{visual_val.read()}",
                                                        oninput: move |evt| visual_val.set(evt.value()),
                                                    }
                                                    button {
                                                        class: "glass-btn btn-sm btn-secondary text-xs",
                                                        onclick: generate_visual_cel,
                                                        "Generate Expression"
                                                    }
                                                }

                                                // CEL Editor & Rule Name
                                                div {
                                                    class: "grid grid-cols-1 md:grid-cols-2 gap-3",
                                                    div {
                                                        class: "form-group",
                                                        label { class: "form-label text-xs", "Rule Name" }
                                                        input {
                                                            r#type: "text",
                                                            class: "glass-input text-xs",
                                                            placeholder: "e.g. Academic Email Matcher",
                                                            value: "{rule_name.read()}",
                                                            oninput: move |evt| rule_name.set(evt.value()),
                                                        }
                                                    }
                                                    div {
                                                        class: "form-group",
                                                        label { class: "form-label text-xs", "CEL Expression (Evaluated to boolean)" }
                                                        textarea {
                                                            class: "glass-input font-mono text-xs h-16 resize-none",
                                                            placeholder: "e.g. email.endsWith('@university.edu') || 'ml-lab' in groups",
                                                            value: "{rule_cel.read()}",
                                                            oninput: move |evt| rule_cel.set(evt.value()),
                                                        }
                                                    }
                                                }

                                                // Interactive Test Sandbox
                                                div {
                                                    class: "flex flex-col gap-2 p-3 rounded-lg bg-black/30 border border-white/5",
                                                    div {
                                                        class: "flex items-center justify-between",
                                                        span { class: "text-xs font-semibold text-muted", "Test Sandbox: Sample OIDC Profile JSON" }
                                                        button {
                                                            class: "glass-btn btn-sm btn-secondary text-xs",
                                                            onclick: test_cel_action,
                                                            IconBolt { size: 12 }
                                                            span { "Test Rule" }
                                                        }
                                                    }
                                                    textarea {
                                                        class: "glass-input font-mono text-xs h-16 resize-none",
                                                        value: "{test_sample_json.read()}",
                                                        oninput: move |evt| test_sample_json.set(evt.value()),
                                                    }
                                                    if let Some((valid, matched, ref err_opt)) = *test_result.read() {
                                                        div {
                                                            class: "text-xs font-mono p-2 rounded flex items-center justify-between",
                                                            style: if !valid { "background: rgba(239, 68, 68, 0.2); color: #f87171;" } else if matched { "background: rgba(34, 197, 94, 0.2); color: #4ade80;" } else { "background: rgba(234, 179, 8, 0.2); color: #facc15;" },
                                                            span {
                                                                if !valid {
                                                                    "{err_opt.as_deref().unwrap_or_default()}"
                                                                } else if matched {
                                                                    "Result: MATCHED (Rule evaluates to true)"
                                                                } else {
                                                                    "Result: DID NOT MATCH (Rule evaluates to false)"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                div {
                                                    class: "flex justify-end mt-2",
                                                    button {
                                                        class: "glass-btn btn-primary text-xs",
                                                        onclick: save_rule_action,
                                                        IconCheck { size: 14 }
                                                        span { "Save Rule" }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Tab 3: Group Rate Limits
                                    if *active_detail_tab.read() == DetailTab::RateLimits {
                                        div {
                                            class: "flex flex-col gap-4",
                                            // Existing group rate limits
                                            div {
                                                class: "glass-table-container max-h-48 overflow-y-auto",
                                                table {
                                                    class: "glass-table",
                                                    thead {
                                                        tr {
                                                            th { "Model" }
                                                            th { "Scope Mode" }
                                                            th { "Threshold" }
                                                            th { "Window" }
                                                            th { "Unit" }
                                                            th { class: "text-right", "Action" }
                                                        }
                                                    }
                                                    tbody {
                                                        if detail.rate_limits.is_empty() {
                                                            tr {
                                                                td {
                                                                    colspan: 6,
                                                                    class: "text-muted text-center py-4",
                                                                    "No specific rate limits assigned to this group."
                                                                }
                                                            }
                                                        } else {
                                                            for rl in detail.rate_limits {
                                                                {
                                                                    let rlid = rl.id.clone();
                                                                    let mode_label = if rl.scope_mode == "shared_group" {
                                                                        "Shared Pool"
                                                                    } else {
                                                                        "Per Member"
                                                                    };
                                                                    rsx! {
                                                                        tr {
                                                                            key: "{rl.id}",
                                                                            td { class: "font-mono text-xs", "{rl.model.as_deref().unwrap_or(\"All Models (*)\")}" }
                                                                            td {
                                                                                GlassBadge { "{mode_label}" }
                                                                            }
                                                                            td { class: "font-semibold", "{format_count(rl.limit_value)}" }
                                                                            td { class: "text-xs", "{rl.time_window}" }
                                                                            td { class: "text-xs", "{rl.unit}" }
                                                                            td {
                                                                                class: "text-right",
                                                                                button {
                                                                                    class: "glass-btn btn-sm btn-danger",
                                                                                    onclick: move |_| delete_rate_limit_action(rlid.clone()),
                                                                                    IconTrash { size: 12 }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Add rate limit form
                                            div {
                                                class: "p-4 rounded-xl border border-white/10 flex flex-col gap-3",
                                                style: "background: var(--glow-surface);",
                                                h4 { class: "text-sm font-bold flex items-center gap-2",
                                                    IconPlus { size: 14 }
                                                    span { "Add Rate Limit for Group" }
                                                }
                                                div {
                                                    class: "grid grid-cols-1 md:grid-cols-3 gap-3",
                                                    div {
                                                        class: "form-group",
                                                        label { class: "form-label text-xs", "Target Model (* for all)" }
                                                        input {
                                                            r#type: "text",
                                                            class: "glass-input text-xs",
                                                            placeholder: "* or model-id",
                                                            value: "{rl_model.read()}",
                                                            oninput: move |evt| rl_model.set(evt.value()),
                                                        }
                                                    }
                                                    div {
                                                        class: "form-group",
                                                        label { class: "form-label text-xs", "Quota Scope Mode" }
                                                        select {
                                                            class: "glass-select text-xs",
                                                            value: "{rl_scope_mode.read()}",
                                                            onchange: move |evt| rl_scope_mode.set(evt.value()),
                                                            option { value: "per_user", "Per-Member Quota (Each member gets limit)" }
                                                            option { value: "shared_group", "Shared Group Pool (Shared across members)" }
                                                        }
                                                    }
                                                    div {
                                                        class: "form-group",
                                                        label { class: "form-label text-xs", "Limit Threshold" }
                                                        input {
                                                            r#type: "text",
                                                            class: "glass-input text-xs",
                                                            placeholder: "e.g. 100000",
                                                            value: "{rl_limit_str.read()}",
                                                            oninput: move |evt| rl_limit_str.set(evt.value()),
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "grid grid-cols-1 md:grid-cols-2 gap-3",
                                                    div {
                                                        class: "form-group",
                                                        label { class: "form-label text-xs", "Time Window" }
                                                        select {
                                                            class: "glass-select text-xs",
                                                            value: "{rl_time_window.read()}",
                                                            onchange: move |evt| rl_time_window.set(evt.value()),
                                                            option { value: "minute", "Minute" }
                                                            option { value: "hour", "Hour" }
                                                            option { value: "day", "Day" }
                                                            option { value: "week", "Week" }
                                                            option { value: "month", "Month" }
                                                            option { value: "permanent", "Permanent (Lifetime)" }
                                                        }
                                                    }
                                                    div {
                                                        class: "form-group",
                                                        label { class: "form-label text-xs", "Unit" }
                                                        select {
                                                            class: "glass-select text-xs",
                                                            value: "{rl_unit.read()}",
                                                            onchange: move |evt| rl_unit.set(evt.value()),
                                                            option { value: "tokens", "Tokens (Total)" }
                                                            option { value: "requests", "Requests" }
                                                            option { value: "prompt_tokens", "Prompt Tokens" }
                                                            option { value: "completion_tokens", "Completion Tokens" }
                                                        }
                                                    }
                                                }
                                                div {
                                                    class: "flex justify-end mt-2",
                                                    button {
                                                        class: "glass-btn btn-primary text-xs",
                                                        onclick: save_group_rate_limit,
                                                        IconCheck { size: 14 }
                                                        span { "Add Rate Limit" }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            } else {
                                div { class: "text-muted text-center py-8", "Loading group details..." }
                            }
                        }
                    }
                }
            }
        }
    }
}
