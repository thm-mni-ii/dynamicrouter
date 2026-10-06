use crate::api::{
    delete_admin_rate_limit, fetch_admin_models, fetch_admin_rate_limit_usage,
    fetch_admin_rate_limits, fetch_admin_users, save_admin_rate_limit,
};
use crate::components::icons::*;
use crate::components::{
    ChipInput, GlassAutocompleteInput, GlassBadge, GlassCard, GlassModal,
};
use crate::models::{AdminModelConfig, AdminUserRecord, RateLimitRecord, RateLimitUsage};
use crate::state::Toast;
use dioxus::prelude::*;

fn parse_limit_shorthand(input: &str) -> Option<i64> {
    let s = input.trim().to_uppercase();
    if s.is_empty() {
        return None;
    }
    if let Some(num_str) = s.strip_suffix('K') {
        let n = num_str.trim().parse::<f64>().ok()?;
        return Some((n * 1_000.0).round() as i64);
    }
    if let Some(num_str) = s.strip_suffix('M') {
        let n = num_str.trim().parse::<f64>().ok()?;
        return Some((n * 1_000_000.0).round() as i64);
    }
    if let Some(num_str) = s.strip_suffix('B') {
        let n = num_str.trim().parse::<f64>().ok()?;
        return Some((n * 1_000_000_000.0).round() as i64);
    }
    s.parse::<i64>().ok()
}

pub fn format_count(val: i64) -> String {
    if val >= 1_000_000_000 {
        format!("{:.2}B", val as f64 / 1_000_000_000.0)
    } else if val >= 1_000_000 {
        format!("{:.1}M", val as f64 / 1_000_000.0)
    } else if val >= 1_000 {
        format!("{:.1}K", val as f64 / 1_000.0)
    } else {
        val.to_string()
    }
}

fn load_rate_limits_action(
    mut is_loading: Signal<bool>,
    mut rules: Signal<Vec<RateLimitRecord>>,
    mut usages: Signal<Vec<RateLimitUsage>>,
    mut users_list: Signal<Vec<AdminUserRecord>>,
    mut models_list: Signal<Vec<AdminModelConfig>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_rate_limits(None, None).await {
            Ok(list) => rules.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load rate limits: {e}"),
                    is_error: true,
                });
            }
        }
        if let Ok(u_list) = fetch_admin_rate_limit_usage(None, None).await {
            usages.set(u_list);
        }
        if let Ok(u) = fetch_admin_users().await {
            users_list.set(u);
        }
        if let Ok(m) = fetch_admin_models().await {
            models_list.set(m);
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminRateLimitsView(toasts: Signal<Vec<Toast>>) -> Element {
    let rules = use_signal(Vec::<RateLimitRecord>::new);
    let usages = use_signal(Vec::<RateLimitUsage>::new);
    let users_list = use_signal(Vec::<AdminUserRecord>::new);
    let models_list = use_signal(Vec::<AdminModelConfig>::new);
    let is_loading = use_signal(|| false);

    let mut editing_rule = use_signal(|| Option::<(RateLimitRecord, bool)>::None);
    let filter_user = use_signal(String::new);
    let mut filter_window = use_signal(String::new);

    // Form fields
    let mut form_users = use_signal(Vec::<String>::new);
    let mut form_models = use_signal(Vec::<String>::new);
    let mut form_window = use_signal(|| "minute".to_string());
    let mut form_unit = use_signal(|| "tokens".to_string());
    let mut form_limit_str = use_signal(|| "100000".to_string());

    let _ = use_resource(move || async move {
        load_rate_limits_action(is_loading, rules, usages, users_list, models_list, toasts);
    });

    let open_create_modal = move |_| {
        form_users.set(Vec::new());
        form_models.set(Vec::new());
        form_window.set("minute".to_string());
        form_unit.set("tokens".to_string());
        form_limit_str.set("100000".to_string());

        editing_rule.set(Some((
            RateLimitRecord {
                id: String::new(),
                user: None,
                model: None,
                time_window: "minute".to_string(),
                limit_value: 100_000,
                unit: "tokens".to_string(),
                created_at: 0,
            },
            true,
        )));
    };

    let save_rule = move |_| {
        let (current, is_new) = match editing_rule.read().clone() {
            Some(r) => r,
            None => return,
        };

        let parsed_limit = match parse_limit_shorthand(&form_limit_str.read()) {
            Some(l) if l > 0 => l,
            _ => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: "Please enter a valid limit value greater than 0 (e.g., 100000, 500k, 10M)".to_string(),
                    is_error: true,
                });
                return;
            }
        };

        let window_val = form_window.read().clone();
        let unit_val = form_unit.read().clone();

        if is_new {
            let u_list = form_users.read().clone();
            let user_targets: Vec<Option<String>> = if u_list.is_empty() || u_list.iter().any(|u| u == "*" || u.trim().is_empty()) {
                vec![None]
            } else {
                u_list.into_iter().filter(|u| !u.trim().is_empty()).map(Some).collect()
            };

            let m_list = form_models.read().clone();
            let model_targets: Vec<Option<String>> = if m_list.is_empty() || m_list.iter().any(|m| m == "*" || m.trim().is_empty()) {
                vec![None]
            } else {
                m_list.into_iter().filter(|m| !m.trim().is_empty()).map(Some).collect()
            };

            spawn(async move {
                let mut created_count = 0;
                let mut err_msg = None;
                for u in user_targets {
                    for m in model_targets.clone() {
                        let record = RateLimitRecord {
                            id: String::new(),
                            user: u.clone(),
                            model: m.clone(),
                            time_window: window_val.clone(),
                            limit_value: parsed_limit,
                            unit: unit_val.clone(),
                            created_at: 0,
                        };
                        match save_admin_rate_limit(&record, true).await {
                            Ok(_) => created_count += 1,
                            Err(e) => err_msg = Some(e),
                        }
                    }
                }

                if let Some(err) = err_msg {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Saved {created_count} rate limit(s), but encountered error: {err}"),
                        is_error: true,
                    });
                } else {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: if created_count == 1 {
                            "Rate limit saved successfully.".to_string()
                        } else {
                            format!("{created_count} rate limit policies created successfully.")
                        },
                        is_error: false,
                    });
                }
                editing_rule.set(None);
                load_rate_limits_action(is_loading, rules, usages, users_list, models_list, toasts);
            });
        } else {
            let u_str = form_users.read().first().cloned().unwrap_or_default();
            let user_val = if u_str.is_empty() || u_str == "*" {
                None
            } else {
                Some(u_str)
            };

            let m_str = form_models.read().first().cloned().unwrap_or_default();
            let model_val = if m_str.is_empty() || m_str == "*" {
                None
            } else {
                Some(m_str)
            };

            let record = RateLimitRecord {
                id: current.id,
                user: user_val,
                model: model_val,
                time_window: window_val,
                limit_value: parsed_limit,
                unit: unit_val,
                created_at: current.created_at,
            };

            spawn(async move {
                match save_admin_rate_limit(&record, false).await {
                    Ok(_) => {
                        toasts.write().push(Toast {
                            id: js_sys::Date::now() as u64,
                            message: "Rate limit updated successfully.".to_string(),
                            is_error: false,
                        });
                        editing_rule.set(None);
                        load_rate_limits_action(is_loading, rules, usages, users_list, models_list, toasts);
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
        }
    };

    let on_delete_rule = move |id: String| {
        spawn(async move {
            match delete_admin_rate_limit(&id).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Rate limit deleted.".to_string(),
                        is_error: false,
                    });
                    load_rate_limits_action(is_loading, rules, usages, users_list, models_list, toasts);
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

    let current_rules = rules.read().clone();
    let current_usages = usages.read().clone();

    // Summary calculations
    let total_rules = current_rules.len();
    let token_rules_count = current_rules.iter().filter(|r| r.unit.contains("token")).count();
    let unique_users_limited = current_rules
        .iter()
        .filter_map(|r| r.user.as_ref())
        .collect::<std::collections::HashSet<_>>()
        .len();

    let filtered_rules: Vec<RateLimitRecord> = current_rules
        .into_iter()
        .filter(|r| {
            let u_filter = filter_user.read().to_lowercase();
            let w_filter = filter_window.read().to_lowercase();

            let user_matches = if u_filter.is_empty() {
                true
            } else {
                r.user
                    .as_deref()
                    .unwrap_or("global")
                    .to_lowercase()
                    .contains(&u_filter)
            };

            let window_matches = if w_filter.is_empty() {
                true
            } else {
                r.time_window.to_lowercase() == w_filter
            };

            user_matches && window_matches
        })
        .collect();

    rsx! {
        div {
            class: "flex flex-col gap-4",

            // Overview Summary Metrics Cards
            div {
                class: "kpi-grid",
                
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Total Quotas" }
                    span { class: "kpi-value", "{total_rules}" }
                    span { class: "kpi-subtext", "Active throttling policies" }
                }

                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Token Compute Rules" }
                    span { class: "kpi-value", "{token_rules_count}" }
                    span { class: "kpi-subtext", "Token-budget enforcement" }
                }

                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Targeted Users" }
                    span { class: "kpi-value", "{unique_users_limited}" }
                    span { class: "kpi-subtext", "Custom user assignments" }
                }

                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Global Throttles" }
                    span {
                        class: "kpi-value",
                        "{total_rules.saturating_sub(unique_users_limited)}"
                    }
                    span { class: "kpi-subtext", "Cluster-wide fair share" }
                }
            }

            // Main Management Card
            GlassCard {
                div {
                    class: "card-header",
                    div {
                        h2 {
                            class: "card-title",
                            IconGauge { size: 20 }
                            span { "Rate Limits & Quota Policies" }
                        }
                        p {
                            class: "card-description",
                            "Configure granular token and request rate limits per minute, hour, day, week, month, or permanent lifetime."
                        }
                    }
                    div {
                        class: "flex items-center gap-2",
                        button {
                            class: "glass-btn btn-primary",
                            onclick: open_create_modal,
                            IconPlus { size: 16 }
                            span { "Add Rate Limit" }
                        }
                    }
                }

                // Filters
                div {
                    class: "flex flex-wrap items-center gap-3 p-3 mb-3",
                    style: "background: var(--glow-surface); border: 1px solid var(--glow-border); border-radius: 10px;",
                    
                    div {
                        class: "flex items-center gap-2",
                        label { class: "text-xs text-muted", "Filter User:" }
                        GlassAutocompleteInput {
                            value: filter_user,
                            suggestions: users_list.read().iter().map(|u| u.username.clone()).collect(),
                            class: "font-mono text-xs",
                            style: "width: 160px;",
                            placeholder: "All users...",
                        }
                    }

                    div {
                        class: "flex items-center gap-2",
                        label { class: "text-xs text-muted", "Window:" }
                        select {
                            class: "glass-select text-xs",
                            style: "width: 140px; padding: 0.25rem 0.5rem;",
                            value: "{filter_window.read()}",
                            onchange: move |evt| filter_window.set(evt.value()),
                            option { value: "", "All Windows" }
                            option { value: "minute", "Per Minute" }
                            option { value: "hour", "Per Hour" }
                            option { value: "day", "Per Day" }
                            option { value: "week", "Per Week" }
                            option { value: "month", "Per Month" }
                            option { value: "permanent", "Permanent" }
                        }
                    }
                }

                // Table
                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Target User" }
                                th { "Target Model" }
                                th { "Time Window" }
                                th { "Unit & Limit" }
                                th { "Current Consumption" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if filtered_rules.is_empty() {
                                tr {
                                    td {
                                        colspan: 6,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading rate limits..." } else { "No rate limit policies configured." }
                                    }
                                }
                            }
                            for rule in filtered_rules.iter() {
                                {
                                    let rule_usage = current_usages.iter().find(|u| u.id == rule.id);
                                    let used = rule_usage.map(|u| u.used).unwrap_or(0);
                                    let limit_val = rule.limit_value;
                                    let pct = if limit_val > 0 {
                                        ((used as f64) / (limit_val as f64) * 100.0).min(100.0) as u32
                                    } else {
                                        0
                                    };
                                    let unit_text = match rule.unit.as_str() {
                                        "requests" => "req",
                                        "prompt_tokens" => "in tokens",
                                        "completion_tokens" => "out tokens",
                                        _ => "tokens",
                                    };

                                    rsx! {
                                        tr {
                                            key: "{rule.id}",
                                            td {
                                                if let Some(ref u) = rule.user {
                                                    span { class: "font-mono font-bold text-sm", "{u}" }
                                                } else {
                                                    GlassBadge { variant: "glow", "* Global (All Users)" }
                                                }
                                            }
                                            td {
                                                if let Some(ref m) = rule.model {
                                                    span { class: "font-mono text-xs", "{m}" }
                                                } else {
                                                    GlassBadge { variant: "secondary", "* All Models" }
                                                }
                                            }
                                            td {
                                                match rule.time_window.as_str() {
                                                    "minute" => rsx! { GlassBadge { variant: "primary", "1 Minute" } },
                                                    "hour" => rsx! { GlassBadge { variant: "info", "1 Hour" } },
                                                    "day" => rsx! { GlassBadge { variant: "success", "1 Day (24h)" } },
                                                    "week" => rsx! { GlassBadge { variant: "warning", "1 Week" } },
                                                    "month" => rsx! { GlassBadge { variant: "glow", "1 Month (30d)" } },
                                                    _ => rsx! { GlassBadge { "Permanent (Lifetime)" } },
                                                }
                                            }
                                            td {
                                                div {
                                                    class: "flex flex-col",
                                                    span { class: "font-mono font-bold text-sm", "{format_count(rule.limit_value)} {unit_text}" }
                                                    span { class: "text-xs text-muted", "{rule.limit_value} raw {rule.unit}" }
                                                }
                                            }
                                            td {
                                                div {
                                                    class: "flex flex-col gap-1",
                                                    style: "min-width: 140px;",
                                                    div {
                                                        class: "flex items-center justify-between text-xs font-mono",
                                                        span { "{format_count(used)} / {format_count(limit_val)}" }
                                                        span { class: if pct > 90 { "text-danger font-bold" } else if pct > 70 { "text-warning" } else { "text-success" }, "{pct}%" }
                                                    }
                                                    div {
                                                        style: "height: 6px; background: var(--glow-surface); border-radius: 3px; overflow: hidden; border: 1px solid var(--glow-border);",
                                                        div {
                                                            style: format!("height: 100%; width: {}%; background: {}; border-radius: 3px; transition: width 0.3s ease;",
                                                                pct,
                                                                if pct > 90 { "var(--glow-danger, #ff4b72)" } else if pct > 70 { "var(--glow-warning, #f59e0b)" } else { "var(--glow-primary, #00f2fe)" }
                                                            )
                                                        }
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
                                                            let r = rule.clone();
                                                            move |_| {
                                                                form_users.set(r.user.as_ref().map(|u| vec![u.clone()]).unwrap_or_default());
                                                                form_models.set(r.model.as_ref().map(|m| vec![m.clone()]).unwrap_or_default());
                                                                form_window.set(r.time_window.clone());
                                                                form_unit.set(r.unit.clone());
                                                                form_limit_str.set(r.limit_value.to_string());
                                                                editing_rule.set(Some((r.clone(), false)));
                                                            }
                                                        },
                                                        "Edit"
                                                    }
                                                    button {
                                                        class: "glass-btn btn-danger btn-sm",
                                                        onclick: {
                                                            let rid = rule.id.clone();
                                                            move |_| on_delete_rule(rid.clone())
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
                }
            }

            // Create / Edit Rate Limit Modal
            if let Some((_, is_new)) = editing_rule.read().clone() {
                {
                    let preview_val = parse_limit_shorthand(&form_limit_str.read());
                    let hint_text = if let Some(pv) = preview_val {
                        let formatted = format_count(pv);
                        format!("Interpreted as: {pv} ({formatted})")
                    } else {
                        "Enter a valid number or shorthand (k, M, B)".to_string()
                    };
                    rsx! {
                        GlassModal {
                            title: if is_new { "Add Rate Limit Policy".to_string() } else { "Edit Rate Limit Policy".to_string() },
                            on_close: move |_| editing_rule.set(None),
                            div {
                                class: "flex flex-col gap-3",

                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Target User(s)" }
                                    ChipInput {
                                        values: form_users,
                                        suggestions: {
                                            let mut list = vec!["*".to_string()];
                                            for u in users_list.read().iter() {
                                                list.push(u.username.clone());
                                            }
                                            list
                                        },
                                        placeholder: "* for All Users, or select specific user(s)...",
                                    }
                                    span { class: "form-hint", "Select username(s) or leave empty/'*' for all users." }
                                }

                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Target Model(s)" }
                                    ChipInput {
                                        values: form_models,
                                        suggestions: {
                                            let mut list = vec!["*".to_string()];
                                            for m in models_list.read().iter() {
                                                list.push(m.id.clone());
                                            }
                                            list
                                        },
                                        placeholder: "* for All Models, or select specific model(s)...",
                                    }
                                    span { class: "form-hint", "Select model(s) or leave empty/'*' for all models." }
                                }

                                div {
                                    class: "grid grid-cols-2 gap-3",
                                    div {
                                        class: "form-group",
                                        label { class: "form-label", "Time Window" }
                                        select {
                                            class: "glass-select font-mono",
                                            value: "{form_window.read()}",
                                            onchange: move |evt| form_window.set(evt.value()),
                                            option { value: "minute", "Minute (60s rolling)" }
                                            option { value: "hour", "Hour (1h rolling)" }
                                            option { value: "day", "Day (24h rolling)" }
                                            option { value: "week", "Week (7d rolling)" }
                                            option { value: "month", "Month (30d rolling)" }
                                            option { value: "permanent", "Permanent (All-Time)" }
                                        }
                                    }

                                    div {
                                        class: "form-group",
                                        label { class: "form-label", "Limit Unit" }
                                        select {
                                            class: "glass-select font-mono",
                                            value: "{form_unit.read()}",
                                            onchange: move |evt| form_unit.set(evt.value()),
                                            option { value: "tokens", "Total Tokens (Recommended)" }
                                            option { value: "prompt_tokens", "Input / Prompt Tokens" }
                                            option { value: "completion_tokens", "Output / Generated Tokens" }
                                            option { value: "requests", "Request Count" }
                                        }
                                    }
                                }

                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Limit Threshold" }
                                    input {
                                        class: "glass-input font-mono",
                                        placeholder: "e.g., 100000, 500k, 10M, 1.5B",
                                        value: "{form_limit_str.read()}",
                                        oninput: move |evt| form_limit_str.set(evt.value()),
                                    }
                                    span {
                                        class: "form-hint font-mono",
                                        "{hint_text}"
                                    }
                                }

                                div {
                                    class: "modal-footer",
                                    button {
                                        class: "glass-btn btn-secondary",
                                        onclick: move |_| editing_rule.set(None),
                                        "Cancel"
                                    }
                                    button {
                                        class: "glass-btn btn-primary",
                                        onclick: save_rule,
                                        "Save Rate Limit"
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
