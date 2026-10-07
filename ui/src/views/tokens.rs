use crate::api::{
    create_user_token, delete_user_token, fetch_user_tokens, fetch_v1_models, update_user_token,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::{
    CreateTokenPayload, CreatedTokenResponse, TokenRateLimitInput, UpdateTokenPayload, UserToken,
};
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::HashSet;
use web_sys::window;

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

fn load_tokens_action(
    mut is_loading: Signal<bool>,
    mut tokens: Signal<Vec<UserToken>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_user_tokens().await {
            Ok(list) => tokens.set(list),
            Err(err) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Error fetching tokens: {err}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn TokensView(toasts: Signal<Vec<Toast>>) -> Element {
    let tokens = use_signal(Vec::<UserToken>::new);
    let is_loading = use_signal(|| false);
    let mut model_suggestions = use_signal(Vec::<String>::new);

    // Modal states
    let mut is_create_modal_open = use_signal(|| false);
    let mut editing_token = use_signal(|| Option::<UserToken>::None);
    let mut created_token = use_signal(|| Option::<CreatedTokenResponse>::None);
    let mut copied = use_signal(|| false);

    // Form inputs for create / edit
    let mut form_name = use_signal(String::new);
    let form_models = use_signal(Vec::<String>::new);
    let mut form_expires_at = use_signal(String::new);
    let form_allowed_ips = use_signal(Vec::<String>::new);
    let mut form_rate_limits = use_signal(Vec::<TokenRateLimitInput>::new);

    let _ = use_resource(move || async move {
        load_tokens_action(is_loading, tokens, toasts);
        if let Ok(models) = fetch_v1_models().await {
            let mut list: Vec<String> = models.into_iter().map(|m| m.id).collect();
            list.sort();
            model_suggestions.set(list);
        }
    });

    let open_create_modal = move |_| {
        let mut form_name = form_name;
        let mut form_models = form_models;
        let mut form_expires_at = form_expires_at;
        let mut form_allowed_ips = form_allowed_ips;
        let mut form_rate_limits = form_rate_limits;
        let mut is_create_modal_open = is_create_modal_open;
        form_name.set(String::new());
        form_models.set(Vec::new());
        form_expires_at.set(String::new());
        form_allowed_ips.set(Vec::new());
        form_rate_limits.set(Vec::new());
        is_create_modal_open.set(true);
    };

    let open_edit_modal = move |tok: UserToken| {
        let mut form_name = form_name;
        let mut form_models = form_models;
        let mut form_expires_at = form_expires_at;
        let mut form_allowed_ips = form_allowed_ips;
        let mut form_rate_limits = form_rate_limits;
        let mut editing_token = editing_token;

        form_name.set(tok.name.clone().unwrap_or_default());
        let mut m_list: Vec<String> = tok
            .limit_to_models
            .as_ref()
            .map(|m| m.iter().cloned().collect())
            .unwrap_or_default();
        m_list.sort();
        form_models.set(m_list);

        if let Some(exp) = tok.expires_at {
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

        form_allowed_ips.set(tok.allowed_ips.clone().unwrap_or_default());

        let rl_inputs: Vec<TokenRateLimitInput> = tok
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

        editing_token.set(Some(tok));
    };

    let parse_expires_at = |exp_str: &str| -> Option<i64> {
        let trimmed = exp_str.trim();
        if trimmed.is_empty() {
            return None;
        }
        let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(trimmed));
        let time = d.get_time();
        if time.is_nan() {
            None
        } else {
            Some((time / 1000.0) as i64)
        }
    };

    let on_submit_create = move |_| {
        let name_trimmed = form_name.read().trim().to_string();
        let name = if name_trimmed.is_empty() {
            None
        } else {
            Some(name_trimmed)
        };

        let models = if form_models.read().is_empty() {
            None
        } else {
            Some(form_models.read().iter().cloned().collect::<HashSet<_>>())
        };

        let expires_at = parse_expires_at(&form_expires_at.read());

        let allowed_ips = if form_allowed_ips.read().is_empty() {
            None
        } else {
            Some(form_allowed_ips.read().clone())
        };

        let rate_limits = if form_rate_limits.read().is_empty() {
            None
        } else {
            Some(form_rate_limits.read().clone())
        };

        let payload = CreateTokenPayload {
            name,
            limit_to_models: models,
            expires_at,
            allowed_ips,
            rate_limits,
        };

        spawn(async move {
            match create_user_token(Some(&payload)).await {
                Ok(res) => {
                    is_create_modal_open.set(false);
                    created_token.set(Some(res));
                    load_tokens_action(is_loading, tokens, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to create token: {err}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let on_submit_edit = move |_| {
        let current_tok = match editing_token.read().clone() {
            Some(t) => t,
            None => return,
        };

        let name_trimmed = form_name.read().trim().to_string();
        let name = if name_trimmed.is_empty() {
            None
        } else {
            Some(name_trimmed)
        };

        let models = if form_models.read().is_empty() {
            None
        } else {
            Some(form_models.read().iter().cloned().collect::<HashSet<_>>())
        };

        let expires_at = parse_expires_at(&form_expires_at.read());

        let allowed_ips = if form_allowed_ips.read().is_empty() {
            None
        } else {
            Some(form_allowed_ips.read().clone())
        };

        let rate_limits = Some(form_rate_limits.read().clone());

        let payload = UpdateTokenPayload {
            name,
            limit_to_models: models,
            expires_at,
            allowed_ips,
            rate_limits,
        };

        let token_hash = current_tok.token_hash.clone();

        spawn(async move {
            match update_user_token(&token_hash, &payload).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Token settings updated successfully.".to_string(),
                        is_error: false,
                    });
                    editing_token.set(None);
                    load_tokens_action(is_loading, tokens, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to update token: {err}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let on_delete_token = move |hash: String| {
        spawn(async move {
            match delete_user_token(&hash).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Token revoked successfully.".to_string(),
                        is_error: false,
                    });
                    load_tokens_action(is_loading, tokens, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to revoke token: {err}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let copy_new_token = {
        move |_| {
            if let Some(res) = created_token.read().as_ref() {
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

    rsx! {
        div {
            class: "flex flex-col gap-4",

            GlassCard {
                div {
                    class: "card-header",
                    div {
                        h2 {
                            class: "card-title",
                            IconKey { size: 20 }
                            span { "Personal API Tokens" }
                        }
                        p { class: "card-description", "Manage your API keys with optional custom names, model restrictions, individual rate limits, IP whitelists, and expiration dates." }
                    }
                    button {
                        class: "glass-btn btn-primary",
                        onclick: open_create_modal,
                        IconPlus { size: 16 }
                        span { "Generate New Token" }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Token Name & Identifier" }
                                th { "Allowed Models" }
                                th { "Token Limits" }
                                th { "Allowed IPs" }
                                th { "Expires" }
                                th { "Created" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if tokens.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 7,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading tokens..." } else { "No API tokens found. Generate one to get started." }
                                    }
                                }
                            }
                            for token in tokens.read().iter() {
                                tr {
                                    key: "{token.token_hash}",
                                    td {
                                        div {
                                            class: "flex flex-col gap-1",
                                            if let Some(ref n) = token.name {
                                                span { class: "font-bold text-sm", "{n}" }
                                            } else {
                                                span { class: "text-sm text-dim italic", "Unnamed Token" }
                                            }
                                            div {
                                                class: "flex items-center gap-1",
                                                span {
                                                    class: "font-mono text-xs px-1.5 py-0.5 rounded",
                                                    style: "background: rgba(255, 255, 255, 0.06); border: 1px solid rgba(255, 255, 255, 0.1); color: #94a3b8;",
                                                    "{token.preview.as_deref().unwrap_or(&token.token_hash)}"
                                                }
                                            }
                                        }
                                    }
                                    td {
                                        if let Some(ref m) = token.limit_to_models {
                                            GlassBadge { variant: "warning", "{m.len()} models allowed" }
                                        } else {
                                            GlassBadge { variant: "success", "All Accessible" }
                                        }
                                    }
                                    td {
                                        div {
                                            class: "flex flex-wrap items-center gap-1",
                                            if token.rate_limits.is_empty() {
                                                span { class: "text-muted text-xs", "—" }
                                            } else {
                                                for rl in token.rate_limits.iter() {
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
                                        if let Some(ref ips) = token.allowed_ips {
                                            if ips.is_empty() {
                                                GlassBadge { "Any IP" }
                                            } else {
                                                GlassBadge { variant: "info", "{ips.len()} ranges" }
                                            }
                                        } else {
                                            GlassBadge { "Any IP" }
                                        }
                                    }
                                    td {
                                        {
                                            if let Some(exp) = token.expires_at {
                                                let now = (js_sys::Date::now() / 1000.0) as i64;
                                                let date_str = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((exp * 1000) as f64))
                                                    .to_locale_string("en-US", &wasm_bindgen::JsValue::UNDEFINED)
                                                    .as_string()
                                                    .unwrap_or_else(|| format!("{exp}"));
                                                if now > exp {
                                                    rsx! { GlassBadge { variant: "danger", "Expired" } }
                                                } else {
                                                    rsx! { GlassBadge { variant: "warning", "{date_str}" } }
                                                }
                                            } else {
                                                rsx! { GlassBadge { "Never" } }
                                            }
                                        }
                                    }
                                    td {
                                        {
                                            if let Some(ts) = token.created_at {
                                                let date_str = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((ts * 1000) as f64))
                                                    .to_locale_date_string("en-US", &wasm_bindgen::JsValue::UNDEFINED)
                                                    .as_string()
                                                    .unwrap_or_else(|| format!("{ts}"));
                                                rsx! { span { class: "text-muted text-xs", "{date_str}" } }
                                            } else {
                                                rsx! { span { class: "text-muted text-xs", "—" } }
                                            }
                                        }
                                    }
                                    td {
                                        class: "text-right",
                                        div {
                                            class: "flex items-center justify-end gap-2",
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let t = token.clone();
                                                    move |_| open_edit_modal(t.clone())
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "glass-btn btn-danger btn-sm",
                                                onclick: {
                                                    let hash = token.token_hash.clone();
                                                    move |_| on_delete_token(hash.clone())
                                                },
                                                IconTrash { size: 14 }
                                                span { "Revoke" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Create Token Modal
            if *is_create_modal_open.read() {
                GlassModal {
                    title: "Generate New API Token".to_string(),
                    on_close: move |_| is_create_modal_open.set(false),
                    div {
                        class: "flex flex-col gap-3",

                        div {
                            class: "form-group",
                            label { class: "form-label", "Token Name (optional)" }
                            input {
                                class: "glass-input",
                                placeholder: "e.g. Cursor IDE, Claude Code, CI Pipeline",
                                value: "{form_name.read()}",
                                oninput: move |evt| form_name.set(evt.value()),
                            }
                            span { class: "form-hint", "A friendly name to identify this token in your dashboard." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Limit to Models (leave empty for access to all permitted models)" }
                            ChipInput {
                                values: form_models,
                                suggestions: model_suggestions.read().clone(),
                                placeholder: "Add allowed model...",
                            }
                            span { class: "form-hint", "Restricts this key to specific models. Cannot exceed your account model permissions." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Allowed IP Ranges / CIDRs (leave empty for any IP)" }
                            ChipInput {
                                values: form_allowed_ips,
                                suggestions: vec!["127.0.0.1/32".to_string(), "192.168.1.0/24".to_string(), "10.0.0.0/8".to_string()],
                                placeholder: "e.g. 192.168.1.0/24, 10.0.0.1",
                            }
                            span { class: "form-hint", "Only requests originating from these IP ranges will be accepted." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Token Expiration (leave empty for never expires)" }
                            input {
                                r#type: "datetime-local",
                                class: "glass-input",
                                value: "{form_expires_at.read()}",
                                oninput: move |evt| form_expires_at.set(evt.value()),
                            }
                            span { class: "form-hint", "Requests using this token will be rejected once this date is reached." }
                        }

                        // Dynamic Multi Rate Limits builder
                        div {
                            class: "flex flex-col gap-2 pt-2 border-t border-glass-border",
                            div {
                                class: "flex items-center justify-between",
                                label { class: "form-label mb-0 font-bold", "Token Rate Limits & Quotas" }
                                button {
                                    r#type: "button",
                                    class: "glass-btn btn-secondary btn-sm",
                                    onclick: add_rate_limit_rule,
                                    IconPlus { size: 14 }
                                    span { "Add Rule" }
                                }
                            }
                            span { class: "form-hint", "Define multi-window rate limits or quotas for this specific key (e.g., 100 req/min, 100k tokens/day). Account limits set by admins still act as a strict ceiling." }

                            if form_rate_limits.read().is_empty() {
                                p { class: "text-xs text-muted italic", "No token-specific rate limits configured (inherits account limits)." }
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
                                onclick: move |_| is_create_modal_open.set(false),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: on_submit_create,
                                "Generate Token"
                            }
                        }
                    }
                }
            }

            // Edit Token Modal
            if let Some(tok) = editing_token.read().clone() {
                GlassModal {
                    title: format!("Edit Token: {}", tok.name.as_deref().unwrap_or(&tok.preview.as_deref().unwrap_or("Token"))),
                    on_close: move |_| editing_token.set(None),
                    div {
                        class: "flex flex-col gap-3",

                        div {
                            class: "form-group",
                            label { class: "form-label", "Token Name" }
                            input {
                                class: "glass-input",
                                value: "{form_name.read()}",
                                oninput: move |evt| form_name.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Limit to Models (leave empty for access to all permitted models)" }
                            ChipInput {
                                values: form_models,
                                suggestions: model_suggestions.read().clone(),
                                placeholder: "Add allowed model...",
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Allowed IP Ranges / CIDRs (leave empty for any IP)" }
                            ChipInput {
                                values: form_allowed_ips,
                                suggestions: vec!["127.0.0.1/32".to_string(), "192.168.1.0/24".to_string(), "10.0.0.0/8".to_string()],
                                placeholder: "e.g. 192.168.1.0/24, 10.0.0.1",
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Token Expiration (leave empty for never expires)" }
                            input {
                                r#type: "datetime-local",
                                class: "glass-input",
                                value: "{form_expires_at.read()}",
                                oninput: move |evt| form_expires_at.set(evt.value()),
                            }
                        }

                        // Dynamic Multi Rate Limits builder
                        div {
                            class: "flex flex-col gap-2 pt-2 border-t border-glass-border",
                            div {
                                class: "flex items-center justify-between",
                                label { class: "form-label mb-0 font-bold", "Token Rate Limits & Quotas" }
                                button {
                                    r#type: "button",
                                    class: "glass-btn btn-secondary btn-sm",
                                    onclick: add_rate_limit_rule,
                                    IconPlus { size: 14 }
                                    span { "Add Rule" }
                                }
                            }

                            if form_rate_limits.read().is_empty() {
                                p { class: "text-xs text-muted italic", "No token-specific rate limits configured (inherits account limits)." }
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
                                onclick: move |_| editing_token.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: on_submit_edit,
                                "Save Changes"
                            }
                        }
                    }
                }
            }

            // New Token Created Modal
            if let Some(res) = created_token.read().clone() {
                GlassModal {
                    title: "Token Generated Successfully".to_string(),
                    on_close: move |_| {
                        created_token.set(None);
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
                                onclick: copy_new_token,
                                if *copied.read() {
                                    IconCheck { size: 14 }
                                    span { "Copied" }
                                } else {
                                    span { "Copy" }
                                }
                            }
                        }
                        div {
                            class: "flex items-start gap-2 p-3 rounded-lg",
                            style: "background: rgba(245, 158, 11, 0.1); border: 1px solid rgba(245, 158, 11, 0.25); color: #fbbf24; font-size: 0.8rem;",
                            IconAlertTriangle { size: 16 }
                            span { "Warning: This token will never be displayed again. Make sure to copy and store it in a secure location." }
                        }
                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| {
                                    created_token.set(None);
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
