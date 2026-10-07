use crate::api::{
    create_token_for_user, delete_admin_user, fetch_admin_users, fetch_v1_models, save_admin_user,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::{AdminUserRecord, CreatedTokenResponse, TokenRateLimitInput};
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::HashSet;
use web_sys::window;

const ONE_WEEK_SECS: i64 = 7 * 24 * 60 * 60;

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

fn format_unit_short(unit: &str) -> &'static str {
    match unit {
        "requests" => "req",
        "prompt_tokens" => "in-tok",
        "completion_tokens" => "out-tok",
        _ => "tok",
    }
}

fn format_window_short(window: &str) -> &str {
    match window {
        "minute" => "min",
        "hour" => "hr",
        "day" => "day",
        "week" => "wk",
        "month" => "mo",
        "permanent" => "total",
        other => other,
    }
}

fn load_users_action(
    mut is_loading: Signal<bool>,
    mut users: Signal<Vec<AdminUserRecord>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_users().await {
            Ok(list) => users.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load users: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminUsersView(toasts: Signal<Vec<Toast>>) -> Element {
    let users = use_signal(Vec::<AdminUserRecord>::new);
    let mut model_suggestions = use_signal(Vec::<String>::new);
    let is_loading = use_signal(|| false);
    let mut editing_user = use_signal(|| Option::<(AdminUserRecord, bool)>::None);
    let mut generated_token = use_signal(|| Option::<CreatedTokenResponse>::None);
    let mut copied = use_signal(|| false);
    let mut show_all_users = use_signal(|| false);

    // Form inputs
    let mut form_username = use_signal(String::new);
    let mut form_is_admin = use_signal(|| false);
    let mut form_log_body = use_signal(|| true);
    let mut form_models = use_signal(Vec::<String>::new);
    let mut form_expires_at = use_signal(String::new);
    let mut form_rate_limits = use_signal(Vec::<TokenRateLimitInput>::new);

    let _ = use_resource(move || async move {
        load_users_action(is_loading, users, toasts);
        if let Ok(models) = fetch_v1_models().await {
            let mut list: Vec<String> = models.into_iter().map(|m| m.id).collect();
            list.sort();
            model_suggestions.set(list);
        }
    });

    let open_create_modal = move |_| {
        form_username.set(String::new());
        form_is_admin.set(false);
        form_log_body.set(true);
        form_models.set(Vec::new());
        form_expires_at.set(String::new());
        form_rate_limits.set(Vec::new());

        editing_user.set(Some((
            AdminUserRecord {
                username: String::new(),
                limit_to_models: None,
                log_request_body: Some(true),
                is_admin: false,
                expires_at: None,
                rate_limits: Vec::new(),
            },
            true,
        )));
    };

    let save_user = move |_| {
        let (_, is_new) = match editing_user.read().clone() {
            Some(u) => u,
            None => return,
        };

        let username = form_username.read().trim().to_string();
        if username.is_empty() {
            return;
        }

        let models = if form_models.read().is_empty() {
            None
        } else {
            Some(form_models.read().iter().cloned().collect::<HashSet<_>>())
        };

        let exp_str = form_expires_at.read().trim().to_string();
        let expires_at = if exp_str.is_empty() {
            None
        } else {
            let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(&exp_str));
            let time = d.get_time();
            if time.is_nan() {
                None
            } else {
                Some((time / 1000.0) as i64)
            }
        };

        let payload = AdminUserRecord {
            username,
            limit_to_models: models,
            log_request_body: Some(*form_log_body.read()),
            is_admin: *form_is_admin.read(),
            expires_at,
            rate_limits: Vec::new(),
        };

        let rate_limits_payload = form_rate_limits.read().clone();

        spawn(async move {
            match save_admin_user(&payload, Some(&rate_limits_payload), is_new).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "User saved successfully.".to_string(),
                        is_error: false,
                    });
                    editing_user.set(None);
                    load_users_action(is_loading, users, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: err,
                        is_error: true,
                    });
                }
            }
        });
    };

    let on_delete_user = move |username: String| {
        spawn(async move {
            match delete_admin_user(&username).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "User deleted.".to_string(),
                        is_error: false,
                    });
                    load_users_action(is_loading, users, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: err,
                        is_error: true,
                    });
                }
            }
        });
    };

    let on_generate_token_for_user = move |username: String| {
        spawn(async move {
            match create_token_for_user(&username, None).await {
                Ok(res) => {
                    generated_token.set(Some(res));
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to generate token: {err}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let copy_gen_token = {
        move |_| {
            if let Some(res) = generated_token.read().as_ref() {
                if let Some(w) = window() {
                    let nav = w.navigator();
                    let clip = nav.clipboard();
                    let _ = clip.write_text(&res.token);
                    copied.set(true);
                }
            }
        }
    };

    let add_rate_limit_rule = move |_| {
        let mut form_rate_limits = form_rate_limits;
        let mut list = form_rate_limits.read().clone();
        list.push(TokenRateLimitInput {
            model: None,
            time_window: "day".to_string(),
            limit_value: 100000,
            unit: Some("tokens".to_string()),
        });
        form_rate_limits.set(list);
    };

    let remove_rate_limit_rule = move |idx: usize| {
        let mut form_rate_limits = form_rate_limits;
        let mut list = form_rate_limits.read().clone();
        if idx < list.len() {
            list.remove(idx);
            form_rate_limits.set(list);
        }
    };

    let now = (js_sys::Date::now() / 1000.0) as i64;
    let show_all = *show_all_users.read();
    let users_list = users.read().clone();
    let filtered_users: Vec<AdminUserRecord> = users_list
        .into_iter()
        .filter(|user| {
            if show_all {
                return true;
            }
            if let Some(exp) = user.expires_at {
                exp + ONE_WEEK_SECS >= now
            } else {
                true
            }
        })
        .collect();

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
                            span { "User & Role Administration" }
                        }
                        p { class: "card-description", "Manage user authorization, admin status, model access restrictions, and generate user API tokens." }
                    }
                    div {
                        class: "flex items-center gap-3",
                        label {
                            class: "flex items-center gap-2 cursor-pointer text-sm text-muted select-none",
                            input {
                                r#type: "checkbox",
                                checked: *show_all_users.read(),
                                onchange: move |evt| show_all_users.set(evt.checked()),
                            }
                            span { "Show all users" }
                        }
                        button {
                            class: "glass-btn btn-primary",
                            onclick: open_create_modal,
                            IconPlus { size: 16 }
                            span { "Add User" }
                        }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Username" }
                                th { "Role" }
                                th { "Model Access" }
                                th { "User Limits" }
                                th { "Logging" }
                                th { "Expires" }
                                th { class: "text-right whitespace-nowrap", "Actions" }
                            }
                        }
                        tbody {
                            if filtered_users.is_empty() {
                                tr {
                                    td {
                                        colspan: 7,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() {
                                            "Loading users..."
                                        } else if users.read().is_empty() {
                                            "No users registered."
                                        } else {
                                            "No active users (expired users older than 1 week are hidden). Check 'Show all users' to view them."
                                        }
                                    }
                                }
                            }
                            for user in filtered_users.iter() {
                                tr {
                                    key: "{user.username}",
                                    td {
                                        span {
                                            class: "font-mono font-bold text-sm",
                                            title: "{user.username}",
                                            style: "max-width: 140px; display: inline-block; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; vertical-align: middle;",
                                            {
                                                if user.username.chars().count() > 16 {
                                                    let truncated: String = user.username.chars().take(16).collect();
                                                    format!("{truncated}…")
                                                } else {
                                                    user.username.clone()
                                                }
                                            }
                                        }
                                    }
                                    td {
                                        if user.is_admin {
                                            GlassBadge { variant: "glow", "Admin" }
                                        } else {
                                            GlassBadge { "User" }
                                        }
                                    }
                                    td {
                                        if let Some(ref m) = user.limit_to_models {
                                            GlassBadge { variant: "warning", "{m.len()} models" }
                                        } else {
                                            GlassBadge { variant: "success", "All Models" }
                                        }
                                    }
                                    td {
                                        div {
                                            class: "flex flex-wrap items-center gap-1",
                                            if user.rate_limits.is_empty() {
                                                span { class: "text-muted text-xs", "Unlimited" }
                                            } else {
                                                for rl in user.rate_limits.iter() {
                                                    GlassBadge {
                                                        key: "{rl.id}",
                                                        span {
                                                            class: "font-mono text-xs",
                                                            if let Some(ref m) = rl.model {
                                                                "{m}: {format_count(rl.used)}/{format_count(rl.limit_value)} {format_unit_short(&rl.unit)}/{format_window_short(&rl.time_window)}"
                                                            } else {
                                                                "{format_count(rl.used)}/{format_count(rl.limit_value)} {format_unit_short(&rl.unit)}/{format_window_short(&rl.time_window)}"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    td {
                                        if user.log_request_body.unwrap_or(true) {
                                            span { class: "text-xs text-muted", "On" }
                                        } else {
                                            span { class: "text-xs text-dim", "Off" }
                                        }
                                    }
                                    td {
                                        {
                                            if let Some(exp) = user.expires_at {
                                                let now = (js_sys::Date::now() / 1000.0) as i64;
                                                let date_str = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((exp * 1000) as f64))
                                                    .to_locale_string("en-US", &wasm_bindgen::JsValue::UNDEFINED)
                                                    .as_string()
                                                    .unwrap_or_else(|| format!("{exp}"));
                                                if now > exp {
                                                    rsx! { GlassBadge { variant: "danger", "Expired ({date_str})" } }
                                                } else {
                                                    rsx! { GlassBadge { variant: "warning", "{date_str}" } }
                                                }
                                            } else {
                                                rsx! { GlassBadge { "Never" } }
                                            }
                                        }
                                    }
                                    td {
                                        class: "text-right whitespace-nowrap",
                                        div {
                                            class: "flex items-center justify-end gap-1.5",
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                style: "padding: 0.3rem 0.6rem;",
                                                onclick: {
                                                    let un = user.username.clone();
                                                    move |_| on_generate_token_for_user(un.clone())
                                                },
                                                IconKey { size: 14 }
                                                span { "Token" }
                                            }
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                style: "padding: 0.3rem 0.65rem;",
                                                onclick: {
                                                    let u = user.clone();
                                                    move |_| {
                                                        form_username.set(u.username.clone());
                                                        form_is_admin.set(u.is_admin);
                                                        form_log_body.set(u.log_request_body.unwrap_or(true));
                                                        let mut m_list: Vec<String> = u.limit_to_models
                                                            .as_ref()
                                                            .map(|m| m.iter().cloned().collect())
                                                            .unwrap_or_default();
                                                        m_list.sort();
                                                        form_models.set(m_list);

                                                        if let Some(exp) = u.expires_at {
                                                            let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((exp * 1000) as f64));
                                                            let iso = d.to_iso_string().as_string().unwrap_or_default();
                                                            if iso.len() >= 16 {
                                                                form_expires_at.set(iso[..16].to_string());
                                                            } else {
                                                                form_expires_at.set(String::new());
                                                            }
                                                        } else {
                                                            form_expires_at.set(String::new());
                                                        }

                                                        let rl_inputs: Vec<TokenRateLimitInput> = u
                                                            .rate_limits
                                                            .iter()
                                                            .map(|r| TokenRateLimitInput {
                                                                model: r.model.clone(),
                                                                time_window: r.time_window.clone(),
                                                                limit_value: r.limit_value,
                                                                unit: Some(r.unit.clone()),
                                                            })
                                                            .collect();
                                                        form_rate_limits.set(rl_inputs);

                                                        editing_user.set(Some((u.clone(), false)));
                                                    }
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "glass-btn btn-danger btn-sm",
                                                style: "padding: 0.3rem 0.65rem;",
                                                onclick: {
                                                    let un = user.username.clone();
                                                    move |_| on_delete_user(un.clone())
                                                },
                                                "Delete"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Edit User Modal
            if let Some((_, is_new)) = editing_user.read().clone() {
                GlassModal {
                    title: if is_new { "Create User".to_string() } else { format!("Edit User: {}", form_username.read()) },
                    on_close: move |_| editing_user.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Username" }
                            input {
                                class: "glass-input font-mono",
                                disabled: !is_new,
                                value: "{form_username.read()}",
                                oninput: move |evt| form_username.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Allowed Models (empty for access to all models)" }
                            ChipInput {
                                values: form_models,
                                suggestions: model_suggestions.read().clone(),
                                placeholder: "Add allowed model...",
                            }
                            span { class: "form-hint", "Select from known models or enter custom model IDs. Comma or Enter to add." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Account Expiration (leave empty for never expires)" }
                            input {
                                r#type: "datetime-local",
                                class: "glass-input",
                                value: "{form_expires_at.read()}",
                                oninput: move |evt| form_expires_at.set(evt.value()),
                            }
                        }

                        div {
                            class: "flex items-center gap-2 mt-2",
                            input {
                                r#type: "checkbox",
                                id: "user-admin-check",
                                checked: *form_is_admin.read(),
                                onchange: move |evt| form_is_admin.set(evt.checked()),
                            }
                            label { r#for: "user-admin-check", class: "text-sm font-bold", "Grant Administrator Privileges" }
                        }

                        div {
                            class: "flex items-center gap-2",
                            input {
                                r#type: "checkbox",
                                id: "user-log-body-check",
                                checked: *form_log_body.read(),
                                onchange: move |evt| form_log_body.set(evt.checked()),
                            }
                            label { r#for: "user-log-body-check", class: "text-sm", "Log Request / Response Body Payloads" }
                        }

                        // Dynamic Multi Rate Limits builder
                        div {
                            class: "flex flex-col gap-2 pt-2 border-t border-glass-border",
                            div {
                                class: "flex items-center justify-between",
                                label { class: "form-label mb-0 font-bold", "User Rate Limits & Quotas" }
                                button {
                                    r#type: "button",
                                    class: "glass-btn btn-secondary btn-sm",
                                    onclick: add_rate_limit_rule,
                                    IconPlus { size: 14 }
                                    span { "Add Rule" }
                                }
                            }
                            span { class: "form-hint", "Define multi-window rate limits or quotas for this user account (e.g., 100 req/min, 100k tokens/day)." }

                            if form_rate_limits.read().is_empty() {
                                p { class: "text-xs text-muted italic", "No user-specific rate limits configured (unlimited by default)." }
                            }

                            for (idx, rl) in form_rate_limits.read().iter().enumerate() {
                                div {
                                    key: "{idx}",
                                    class: "p-2.5 rounded-lg flex flex-col gap-2",
                                    style: "background: var(--glass-bg-subtle); border: 1px solid var(--glass-border);",
                                    div {
                                        class: "grid grid-cols-3 gap-2",
                                        div {
                                            label { class: "text-xs text-muted", "Limit Value" }
                                            input {
                                                r#type: "number",
                                                class: "glass-input text-xs",
                                                value: "{rl.limit_value}",
                                                oninput: move |evt| {
                                                    let val = evt.value().parse::<i64>().unwrap_or(0);
                                                    let mut list = form_rate_limits.read().clone();
                                                    if let Some(item) = list.get_mut(idx) {
                                                        item.limit_value = val;
                                                        form_rate_limits.set(list);
                                                    }
                                                },
                                            }
                                        }
                                        div {
                                            label { class: "text-xs text-muted", "Unit" }
                                            select {
                                                class: "glass-input text-xs",
                                                value: "{rl.unit.as_deref().unwrap_or(\"tokens\")}",
                                                onchange: move |evt| {
                                                    let val = evt.value();
                                                    let mut list = form_rate_limits.read().clone();
                                                    if let Some(item) = list.get_mut(idx) {
                                                        item.unit = Some(val);
                                                        form_rate_limits.set(list);
                                                    }
                                                },
                                                option { value: "tokens", "Total Tokens" }
                                                option { value: "requests", "Requests" }
                                                option { value: "prompt_tokens", "Prompt Tokens" }
                                                option { value: "completion_tokens", "Completion Tokens" }
                                            }
                                        }
                                        div {
                                            label { class: "text-xs text-muted", "Window" }
                                            select {
                                                class: "glass-input text-xs",
                                                value: "{rl.time_window}",
                                                onchange: move |evt| {
                                                    let val = evt.value();
                                                    let mut list = form_rate_limits.read().clone();
                                                    if let Some(item) = list.get_mut(idx) {
                                                        item.time_window = val;
                                                        form_rate_limits.set(list);
                                                    }
                                                },
                                                option { value: "minute", "Per Minute" }
                                                option { value: "hour", "Per Hour" }
                                                option { value: "day", "Per Day" }
                                                option { value: "week", "Per Week" }
                                                option { value: "month", "Per Month" }
                                                option { value: "permanent", "Permanent (Total)" }
                                            }
                                        }
                                    }
                                    div {
                                        class: "flex items-center justify-between",
                                        div {
                                            class: "flex items-center gap-1",
                                            label { class: "text-xs text-muted", "Model (optional):" }
                                            input {
                                                class: "glass-input text-xs",
                                                style: "width: 180px; padding: 2px 6px;",
                                                placeholder: "* (all models)",
                                                value: "{rl.model.as_deref().unwrap_or_default()}",
                                                oninput: move |evt| {
                                                    let v = evt.value().trim().to_string();
                                                    let mut list = form_rate_limits.read().clone();
                                                    if let Some(item) = list.get_mut(idx) {
                                                        item.model = if v.is_empty() || v == "*" { None } else { Some(v) };
                                                        form_rate_limits.set(list);
                                                    }
                                                }
                                            }
                                        }
                                        button {
                                            r#type: "button",
                                            class: "glass-btn btn-danger btn-sm",
                                            style: "padding: 2px 6px; font-size: 0.75rem;",
                                            onclick: move |_| remove_rate_limit_rule(idx),
                                            "Remove"
                                        }
                                    }
                                }
                            }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| editing_user.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_user,
                                "Save User"
                            }
                        }
                    }
                }
            }

            // Token Generated on Behalf of User Modal
            if let Some(res) = generated_token.read().clone() {
                GlassModal {
                    title: format!("Token Generated for {}", res.username),
                    on_close: move |_| {
                        generated_token.set(None);
                        copied.set(false);
                    },
                    div {
                        class: "flex flex-col gap-3",
                        p { class: "text-sm text-muted", "{res.message}" }
                        div {
                            class: "flex items-center gap-2",
                            input {
                                class: "glass-input font-mono text-sm",
                                readonly: true,
                                value: "{res.token}",
                            }
                            button {
                                class: "glass-btn btn-primary btn-sm",
                                onclick: copy_gen_token,
                                if *copied.read() {
                                    IconCheck { size: 14 }
                                    span { "Copied" }
                                } else {
                                    span { "Copy" }
                                }
                            }
                        }
                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| {
                                    generated_token.set(None);
                                    copied.set(false);
                                },
                                "Done"
                            }
                        }
                    }
                }
            }
        }
    }
}
