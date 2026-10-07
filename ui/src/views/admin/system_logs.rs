use crate::api::{
    cleanup_admin_system_logs, fetch_admin_system_logs, fetch_admin_users, fetch_v1_models,
};
use crate::components::icons::*;
use crate::components::{
    CodeBlock, GlassAutocompleteInput, GlassBadge, GlassCard, GlassModal,
};
use crate::models::{PaginatedRequests, RequestLog};
use crate::state::Toast;
use dioxus::prelude::*;

fn format_timestamp(ts: i64) -> String {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((ts * 1000) as f64));
    date.to_locale_string("en-US", &wasm_bindgen::JsValue::UNDEFINED)
        .as_string()
        .unwrap_or_else(|| format!("{ts}"))
}

fn load_system_logs_action(
    mut is_loading: Signal<bool>,
    mut logs_data: Signal<Option<PaginatedRequests>>,
    filter_user: Signal<String>,
    filter_model: Signal<String>,
    page: Signal<i64>,
) {
    is_loading.set(true);
    let u = filter_user.read().clone();
    let m = filter_model.read().clone();
    let user_param = if u.trim().is_empty() { None } else { Some(u) };
    let model_param = if m.trim().is_empty() { None } else { Some(m) };
    let p = *page.read();

    spawn(async move {
        match fetch_admin_system_logs(user_param.as_deref(), model_param.as_deref(), Some(p), Some(50)).await {
            Ok(data) => logs_data.set(Some(data)),
            Err(_) => logs_data.set(None),
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminSystemLogsView(toasts: Signal<Vec<Toast>>) -> Element {
    let logs_data = use_signal(|| Option::<PaginatedRequests>::None);
    let mut user_suggestions = use_signal(Vec::<String>::new);
    let mut model_suggestions = use_signal(Vec::<String>::new);
    let filter_user = use_signal(String::new);
    let filter_model = use_signal(String::new);
    let mut page = use_signal(|| 1i64);
    let is_loading = use_signal(|| false);
    let mut selected_req = use_signal(|| Option::<RequestLog>::None);
    let mut show_cleanup_modal = use_signal(|| false);
    let mut cleanup_days = use_signal(|| 7i64);

    let _ = use_resource(move || async move {
        load_system_logs_action(is_loading, logs_data, filter_user, filter_model, page);

        if let Ok(users) = fetch_admin_users().await {
            let mut list: Vec<String> = users.into_iter().map(|u| u.username).collect();
            list.sort();
            user_suggestions.set(list);
        }

        if let Ok(models) = fetch_v1_models().await {
            let mut list: Vec<String> = models.into_iter().map(|m| m.id).collect();
            list.sort();
            model_suggestions.set(list);
        }
    });

    let on_cleanup = move |_| {
        let secs = *cleanup_days.read() * 86400;
        let older_than_timestamp = (js_sys::Date::now() / 1000.0) as i64 - secs;

        spawn(async move {
            match cleanup_admin_system_logs(older_than_timestamp).await {
                Ok(deleted_count) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Cleanup complete. Removed {deleted_count} old request logs."),
                        is_error: false,
                    });
                    show_cleanup_modal.set(false);
                    load_system_logs_action(is_loading, logs_data, filter_user, filter_model, page);
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

    rsx! {
        div {
            class: "flex flex-col gap-4",
            
            GlassCard {
                div {
                    class: "card-header",
                    div {
                        h2 {
                            class: "card-title",
                            IconAudit { size: 20 }
                            span { "System Request Audit Logs" }
                        }
                        p { class: "card-description", "Complete audit trail of all model inference requests across all users and API tokens." }
                    }
                    div {
                        class: "flex items-center gap-2",
                        button {
                            class: "glass-btn btn-danger btn-sm",
                            onclick: move |_| show_cleanup_modal.set(true),
                            IconTrash { size: 14 }
                            span { "Clean Old Logs" }
                        }
                        button {
                            class: "glass-btn btn-secondary btn-sm",
                            onclick: move |_| load_system_logs_action(is_loading, logs_data, filter_user, filter_model, page),
                            IconRefresh {
                                size: 14,
                                class: if *is_loading.read() { "animate-spin" } else { "" }
                            }
                            span { if *is_loading.read() { "Refreshing..." } else { "Refresh" } }
                        }
                    }
                }

                // Filter Bar
                div {
                    class: "flex items-center gap-3 mb-4",
                    div {
                        style: "width: 200px;",
                        GlassAutocompleteInput {
                            value: filter_user,
                            suggestions: user_suggestions.read().clone(),
                            placeholder: "Filter by User...",
                            on_change: move |_| {
                                page.set(1);
                            },
                        }
                    }
                    div {
                        style: "width: 240px;",
                        GlassAutocompleteInput {
                            value: filter_model,
                            suggestions: model_suggestions.read().clone(),
                            placeholder: "Filter by Model...",
                            on_change: move |_| {
                                page.set(1);
                            },
                        }
                    }
                    button {
                        class: "glass-btn btn-primary btn-sm",
                        onclick: move |_| load_system_logs_action(is_loading, logs_data, filter_user, filter_model, page),
                        "Apply Filters"
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Request ID" }
                                th { "User" }
                                th { "Model" }
                                th { "Enqueue" }
                                th { "Duration" }
                                th { "Status" }
                                th { class: "text-right", "Details" }
                            }
                        }
                        tbody {
                            if let Some(ref data) = logs_data.read().as_ref() {
                                if data.requests.is_empty() {
                                    tr {
                                        td {
                                            colspan: 7,
                                            class: "text-muted text-center",
                                            style: "padding: 2rem;",
                                            "No system logs match the current filter criteria."
                                        }
                                    }
                                }
                                for req in data.requests.iter() {
                                    tr {
                                        key: "{req.id}",
                                        td { span { class: "font-mono text-xs", "{req.id}" } }
                                        td { span { class: "font-mono font-bold text-xs", "{req.user.as_deref().unwrap_or(\"Anonymous\")}" } }
                                        td { span { class: "font-bold text-sm", "{req.model}" } }
                                        td { span { class: "text-xs text-muted", "{format_timestamp(req.enqueue)}" } }
                                        td {
                                            span { class: "text-sm",
                                                if let Some(done) = req.done {
                                                    "{done - req.enqueue}s"
                                                } else {
                                                    "—"
                                                }
                                            }
                                        }
                                        td {
                                            if req.done.is_some() {
                                                GlassBadge { variant: "success", "Done" }
                                            } else {
                                                GlassBadge { variant: "warning", pulse: true, "In Flight" }
                                            }
                                        }
                                        td {
                                            class: "text-right",
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let r = req.clone();
                                                    move |_| selected_req.set(Some(r.clone()))
                                                },
                                                "Inspect"
                                            }
                                        }
                                    }
                                }
                            } else {
                                tr {
                                    td {
                                        colspan: 7,
                                        class: "text-muted text-center",
                                        style: "padding: 2rem;",
                                        if *is_loading.read() { "Loading system logs..." } else { "No data available." }
                                    }
                                }
                            }
                        }
                    }
                }

                // Pagination
                if let Some(ref data) = logs_data.read().as_ref() {
                    div {
                        class: "flex items-center justify-between mt-4",
                        span { class: "text-xs text-muted", "Showing page {data.page} (Total: {data.total} logs)" }
                        div {
                            class: "flex items-center gap-2",
                            button {
                                class: "glass-btn btn-secondary btn-sm",
                                disabled: *page.read() <= 1,
                                onclick: move |_| {
                                    let cur = *page.read();
                                    if cur > 1 {
                                        page.set(cur - 1);
                                        load_system_logs_action(is_loading, logs_data, filter_user, filter_model, page);
                                    }
                                },
                                "← Previous"
                            }
                            button {
                                class: "glass-btn btn-secondary btn-sm",
                                disabled: (*page.read() * data.page_size) >= data.total,
                                onclick: move |_| {
                                    let cur = *page.read();
                                    page.set(cur + 1);
                                    load_system_logs_action(is_loading, logs_data, filter_user, filter_model, page);
                                },
                                "Next →"
                            }
                        }
                    }
                }
            }

            // Inspect Payload Modal
            if let Some(req) = selected_req.read().clone() {
                GlassModal {
                    title: format!("Audit Inspector: {}", req.id),
                    on_close: move |_| selected_req.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        div {
                            class: "kpi-grid",
                            style: "grid-template-columns: repeat(4, 1fr); margin-bottom: 0;",
                            div {
                                class: "kpi-card",
                                span { class: "kpi-title", "User" }
                                span { class: "text-sm font-bold font-mono", "{req.user.as_deref().unwrap_or(\"Anonymous\")}" }
                            }
                            div {
                                class: "kpi-card",
                                span { class: "kpi-title", "Model" }
                                span { class: "text-sm font-bold", "{req.model}" }
                            }
                            div {
                                class: "kpi-card",
                                span { class: "kpi-title", "Duration" }
                                span { class: "text-sm font-bold",
                                    if let Some(done) = req.done {
                                        "{done - req.enqueue}s"
                                    } else {
                                        "In Flight"
                                    }
                                }
                            }
                            div {
                                class: "kpi-card",
                                span { class: "kpi-title", "Instance" }
                                span { class: "text-sm font-bold", "{req.instance.as_deref().unwrap_or(\"Default\")}" }
                            }
                        }

                        if let Some(ref req_body) = req.request_body {
                            CodeBlock {
                                title: Some("Request Body Payload".to_string()),
                                code: serde_json::to_string_pretty(req_body).unwrap_or_default(),
                            }
                        }

                        if let Some(ref res_body) = req.response_body {
                            CodeBlock {
                                title: Some("Response Body Payload".to_string()),
                                code: serde_json::to_string_pretty(res_body).unwrap_or_default(),
                            }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| selected_req.set(None),
                                "Close"
                            }
                        }
                    }
                }
            }

            // Cleanup Modal
            if *show_cleanup_modal.read() {
                GlassModal {
                    title: "Retention Cleanup: Prune Old Logs".to_string(),
                    on_close: move |_| show_cleanup_modal.set(false),
                    div {
                        class: "flex flex-col gap-3",
                        p { class: "text-sm text-muted", "Permanently delete historical request logs older than the specified duration to reclaim database storage." }
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Delete logs older than" }
                            select {
                                class: "glass-select",
                                value: "{cleanup_days.read()}",
                                onchange: move |evt| {
                                    if let Ok(v) = evt.value().parse::<i64>() {
                                        cleanup_days.set(v);
                                    }
                                },
                                option { value: "1", "1 Day (24 Hours)" }
                                option { value: "7", "7 Days (1 Week)" }
                                option { value: "14", "14 Days (2 Weeks)" }
                                option { value: "30", "30 Days (1 Month)" }
                                option { value: "90", "90 Days (3 Months)" }
                            }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| show_cleanup_modal.set(false),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-danger",
                                onclick: on_cleanup,
                                "Execute Cleanup"
                            }
                        }
                    }
                }
            }
        }
    }
}
