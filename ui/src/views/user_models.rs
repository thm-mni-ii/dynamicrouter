use crate::api::fetch_my_models;
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard};
use crate::models::UserModelStatus;
use crate::state::ActiveTab;
use crate::views::admin::format_count;
use dioxus::prelude::*;

fn load_models_action(
    mut is_loading: Signal<bool>,
    mut models: Signal<Vec<UserModelStatus>>,
) {
    is_loading.set(true);
    spawn(async move {
        if let Ok(list) = fetch_my_models().await {
            models.set(list);
        }
        is_loading.set(false);
    });
}

#[component]
pub fn UserModelsView(mut active_tab: Signal<ActiveTab>) -> Element {
    let mut models = use_signal(Vec::<UserModelStatus>::new);
    let is_loading = use_signal(|| false);
    let mut search_query = use_signal(String::new);
    let mut status_filter = use_signal(|| "all".to_string());
    let mut auto_refresh = use_signal(|| true);
    let mut view_mode = use_signal(|| "cards".to_string()); // "cards" or "table"

    // Initial load
    let _ = use_resource(move || async move {
        load_models_action(is_loading, models);
    });

    // Auto-refresh timer loop (every 5 seconds when auto_refresh is enabled)
    use_future(move || async move {
        loop {
            gloo_timers::future::TimeoutFuture::new(5000).await;
            if *auto_refresh.read() {
                if let Ok(list) = fetch_my_models().await {
                    models.set(list);
                }
            }
        }
    });

    let model_list = models.read().clone();

    // Summary calculations
    let total_models = model_list.len();
    let running_models = model_list.iter().filter(|m| m.status == "running").count();
    let upstream_models = model_list.iter().filter(|m| m.status == "upstream").count();
    let total_queue: i64 = model_list.iter().map(|m| m.total_queue_length).sum();
    let user_queue: i64 = model_list.iter().map(|m| m.user_queue_length).sum();
    let cluster_tps: f64 = model_list.iter().map(|m| m.total_output_tps).sum();
    let user_tps: f64 = model_list.iter().map(|m| m.user_output_tps).sum();

    let user_tps_share = if cluster_tps > 0.0 {
        (user_tps / cluster_tps * 100.0).min(100.0)
    } else {
        0.0
    };

    // Filtering
    let search = search_query.read().to_lowercase().trim().to_string();
    let filter = status_filter.read().clone();

    let filtered_models: Vec<UserModelStatus> = model_list
        .into_iter()
        .filter(|m| {
            if filter != "all" && m.status != filter {
                return false;
            }
            if !search.is_empty() {
                let matches_name = m.name.to_lowercase().contains(&search);
                let matches_id = m.id.to_lowercase().contains(&search);
                let matches_alias = m.aliases.iter().any(|a| a.to_lowercase().contains(&search));
                if !matches_name && !matches_id && !matches_alias {
                    return false;
                }
            }
            true
        })
        .collect();

    rsx! {
        div {
            class: "flex flex-col gap-4",
            
            // Header Bar
            div {
                class: "flex flex-wrap items-center justify-between gap-3",
                div {
                    h2 {
                        class: "card-title",
                        IconBot { size: 20 }
                        span { "Model Catalog & Cluster Status" }
                    }
                    p { class: "card-description", "Real-time health, instance scaling, queues, and output throughput for models accessible to you." }
                }
                div {
                    class: "flex items-center gap-2",
                    
                    // View Mode Toggle
                    button {
                        class: "glass-btn btn-secondary btn-sm",
                        onclick: move |_| {
                            let cur = view_mode.read().clone();
                            if cur == "cards" {
                                view_mode.set("table".to_string());
                            } else {
                                view_mode.set("cards".to_string());
                            }
                        },
                        IconSliders { size: 14 }
                        span { if *view_mode.read() == "cards" { "Table View" } else { "Cards View" } }
                    }

                    // Auto-refresh toggle
                    button {
                        class: if *auto_refresh.read() { "glass-btn btn-primary btn-sm" } else { "glass-btn btn-secondary btn-sm" },
                        onclick: move |_| {
                            let cur = *auto_refresh.read();
                            auto_refresh.set(!cur);
                        },
                        IconClock { size: 14 }
                        span { if *auto_refresh.read() { "Live Polling (5s)" } else { "Auto-Refresh Off" } }
                    }

                    // Refresh Button
                    button {
                        class: "glass-btn btn-secondary btn-sm",
                        onclick: move |_| load_models_action(is_loading, models),
                        IconRefresh {
                            size: 14,
                            class: if *is_loading.read() { "animate-spin" } else { "" }
                        }
                        span { if *is_loading.read() { "Refreshing..." } else { "Refresh" } }
                    }
                }
            }

            // Top KPI Summary Grid
            div {
                class: "kpi-grid",
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Accessible Models" }
                    span { class: "kpi-value", "{total_models}" }
                    span { class: "kpi-subtext", "{running_models} warm, {upstream_models} cloud API" }
                }
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Active Cluster Queue" }
                    span { class: "kpi-value", "{total_queue}" }
                    span {
                        class: "kpi-subtext",
                        if user_queue > 0 {
                            "{user_queue} requests from your account"
                        } else {
                            "0 requests queued from you"
                        }
                    }
                }
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Cluster Throughput" }
                    span { class: "kpi-value", "{cluster_tps:.1} tp/s" }
                    span { class: "kpi-subtext", "Total output tokens per second (last 60s)" }
                }
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Your Throughput" }
                    span { class: "kpi-value", "{user_tps:.1} tp/s" }
                    span {
                        class: "kpi-subtext",
                        if cluster_tps > 0.0 {
                            "{user_tps_share:.1}% of cluster output"
                        } else {
                            "0.0% of cluster output"
                        }
                    }
                }
            }

            // Search & Filter Bar
            GlassCard {
                class: "p-3",
                div {
                    class: "flex flex-wrap items-center justify-between gap-3",
                    
                    // Search Input
                    div {
                        class: "flex items-center gap-2",
                        style: "min-width: 260px; flex: 1;",
                        input {
                            r#type: "text",
                            class: "glass-input",
                            placeholder: "Search by model name, ID, or alias...",
                            value: "{search_query.read()}",
                            oninput: move |evt| search_query.set(evt.value()),
                        }
                    }

                    // Filter Buttons
                    div {
                        class: "flex items-center gap-1.5 flex-wrap",
                        button {
                            class: if *status_filter.read() == "all" { "glass-btn btn-primary btn-sm" } else { "glass-btn btn-secondary btn-sm" },
                            onclick: move |_| status_filter.set("all".to_string()),
                            "All ({total_models})"
                        }
                        button {
                            class: if *status_filter.read() == "running" { "glass-btn btn-primary btn-sm" } else { "glass-btn btn-secondary btn-sm" },
                            onclick: move |_| status_filter.set("running".to_string()),
                            "🟢 Ready ({running_models})"
                        }
                        button {
                            class: if *status_filter.read() == "idle" { "glass-btn btn-primary btn-sm" } else { "glass-btn btn-secondary btn-sm" },
                            onclick: move |_| status_filter.set("idle".to_string()),
                            "⚪ Cold ({total_models.saturating_sub(running_models + upstream_models)})"
                        }
                        button {
                            class: if *status_filter.read() == "upstream" { "glass-btn btn-primary btn-sm" } else { "glass-btn btn-secondary btn-sm" },
                            onclick: move |_| status_filter.set("upstream".to_string()),
                            "🟣 Upstream ({upstream_models})"
                        }
                    }
                }
            }

            // Models Display: Cards Grid or Table
            if filtered_models.is_empty() {
                GlassCard {
                    div {
                        class: "text-muted",
                        style: "padding: 3rem; text-align: center;",
                        if *is_loading.read() {
                            "Loading models and operational metrics..."
                        } else {
                            "No models found matching your search and filter criteria."
                        }
                    }
                }
            } else if *view_mode.read() == "table" {
                // Table View
                GlassCard {
                    div {
                        class: "glass-table-container",
                        table {
                            class: "glass-table",
                            thead {
                                tr {
                                    th { "Model" }
                                    th { "Status" }
                                    th { "Instances" }
                                    th { "Queue" }
                                    th { "Total Output" }
                                    th { "Your Share" }
                                    th { "Latency" }
                                    th { "Context" }
                                    th { class: "text-right", "Action" }
                                }
                            }
                            tbody {
                                for m in filtered_models.iter() {
                                    tr {
                                        key: "{m.id}",
                                        td {
                                            div {
                                                class: "flex flex-col gap-0.5",
                                                span { class: "font-bold text-sm", "{m.name}" }
                                                span { class: "font-mono text-xs text-muted", "{m.id}" }
                                                if !m.aliases.is_empty() {
                                                    span { class: "text-xs text-dim", "alias: {m.aliases.join(\", \")}" }
                                                }
                                            }
                                        }
                                        td {
                                            if m.status == "running" {
                                                GlassBadge { variant: "glow", "🟢 Ready" }
                                            } else if m.status == "upstream" {
                                                GlassBadge { variant: "primary", "🟣 Cloud API" }
                                            } else {
                                                GlassBadge { variant: "secondary", "⚪ Cold Start" }
                                            }
                                        }
                                        td {
                                            span {
                                                class: "font-mono text-sm",
                                                if m.handler_type == "openai" {
                                                    "—"
                                                } else if let (Some(min_s), Some(max_s)) = (m.min_scale, m.max_scale) {
                                                    "{m.active_instances} ({min_s}–{max_s})"
                                                } else {
                                                    "{m.active_instances}"
                                                }
                                            }
                                        }
                                        td {
                                            div {
                                                class: "flex items-center gap-1",
                                                span { class: "font-mono text-sm font-bold", "{m.total_queue_length}" }
                                                if m.user_queue_length > 0 {
                                                    span {
                                                        class: "text-xs px-1.5 py-0.5 rounded font-bold text-danger",
                                                        style: "background: rgba(255, 75, 114, 0.15);",
                                                        "{m.user_queue_length} you"
                                                    }
                                                }
                                            }
                                        }
                                        td {
                                            span { class: "font-mono text-sm font-bold", "{m.total_output_tps:.1} tp/s" }
                                        }
                                        td {
                                            span {
                                                class: if m.user_output_tps > 0.0 { "font-mono text-sm font-bold text-primary" } else { "font-mono text-sm text-muted" },
                                                "{m.user_output_tps:.1} tp/s"
                                            }
                                        }
                                        td {
                                            span {
                                                class: "font-mono text-sm",
                                                if let Some(avg) = m.avg_duration_secs {
                                                    "{avg:.2}s"
                                                } else {
                                                    "—"
                                                }
                                            }
                                        }
                                        td {
                                            span {
                                                class: "font-mono text-xs text-dim",
                                                if let Some(ctx) = m.ctx_length {
                                                    "{format_count(ctx as i64)}"
                                                } else {
                                                    "—"
                                                }
                                            }
                                        }
                                        td {
                                            class: "text-right",
                                            button {
                                                class: "glass-btn btn-primary btn-sm",
                                                onclick: move |_| active_tab.set(ActiveTab::Chat),
                                                IconChat { size: 13 }
                                                span { "Chat" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                // Cards Grid View
                div {
                    class: "models-grid",
                    for m in filtered_models.iter() {
                        {
                            let model_id = m.id.clone();
                            let model_name = m.name.clone();
                            let is_running = m.status == "running";
                            let is_upstream = m.status == "upstream";
                            let is_idle = m.status == "idle";

                            rsx! {
                                div {
                                    key: "{model_id}",
                                    class: "model-status-card",
                                    
                                    // Card Header
                                    div {
                                        class: "model-card-header",
                                        div {
                                            class: "model-card-title-group",
                                            h3 { class: "model-card-name", "{model_name}" }
                                            span { class: "model-card-id", "{model_id}" }
                                        }
                                        
                                        // Status Badge
                                        if is_running {
                                            GlassBadge {
                                                variant: "glow",
                                                "🟢 Ready ({m.active_instances})"
                                            }
                                        } else if is_upstream {
                                            GlassBadge {
                                                variant: "primary",
                                                "🟣 Cloud API"
                                            }
                                        } else {
                                            GlassBadge {
                                                variant: "secondary",
                                                "⚪ Cold Start"
                                            }
                                        }
                                    }

                                    // Aliases row if present
                                    if !m.aliases.is_empty() {
                                        div {
                                            class: "flex items-center gap-1.5 flex-wrap",
                                            span { class: "text-xs text-dim font-bold", "Aliases:" }
                                            for alias in m.aliases.iter() {
                                                span {
                                                    key: "{alias}",
                                                    class: "text-xs font-mono px-2 py-0.5 rounded",
                                                    style: "background: var(--glow-surface); border: 1px solid var(--glow-border);",
                                                    "{alias}"
                                                }
                                            }
                                        }
                                    }

                                    // 4-Column Stats Grid
                                    div {
                                        class: "model-card-stats-grid",
                                        
                                        // Total Output Throughput
                                        div {
                                            class: "model-stat-box",
                                            span { class: "model-stat-label", "Total Output" }
                                            span { class: "model-stat-value", "{m.total_output_tps:.1} tp/s" }
                                        }

                                        // Your Output Throughput
                                        div {
                                            class: "model-stat-box",
                                            span { class: "model-stat-label", "Your Share" }
                                            span {
                                                class: if m.user_output_tps > 0.0 { "model-stat-value text-primary" } else { "model-stat-value text-muted" },
                                                "{m.user_output_tps:.1} tp/s"
                                            }
                                        }

                                        // Queue Depth
                                        div {
                                            class: "model-stat-box",
                                            span { class: "model-stat-label", "Queue Depth" }
                                            div {
                                                class: "flex items-center gap-1",
                                                span { class: "model-stat-value", "{m.total_queue_length}" }
                                                if m.user_queue_length > 0 {
                                                    span {
                                                        class: "text-xs px-1.5 py-0.5 rounded font-bold text-danger",
                                                        style: "background: rgba(255, 75, 114, 0.18);",
                                                        "({m.user_queue_length} you)"
                                                    }
                                                }
                                            }
                                        }

                                        // Latency
                                        div {
                                            class: "model-stat-box",
                                            span { class: "model-stat-label", "Avg Latency" }
                                            span {
                                                class: "model-stat-value",
                                                if let Some(avg) = m.avg_duration_secs {
                                                    "{avg:.2}s"
                                                } else {
                                                    "—"
                                                }
                                            }
                                        }
                                    }

                                    // Share Meter (if throughput > 0)
                                    if m.total_output_tps > 0.0 {
                                        div {
                                            class: "model-share-bar-container",
                                            div {
                                                class: "flex items-center justify-between text-xs text-muted",
                                                span { "Your Output Attribution" }
                                                span { class: "font-mono font-bold text-primary", "{m.user_tps_share_pct:.1}%" }
                                            }
                                            div {
                                                class: "model-share-bar-track",
                                                div {
                                                    class: "model-share-bar-fill",
                                                    style: format!("width: {}%;", m.user_tps_share_pct.min(100.0)),
                                                }
                                            }
                                        }
                                    }

                                    // Specs row (Scaling, Context, Lifetime)
                                    div {
                                        class: "model-card-specs",
                                        div {
                                            class: "flex items-center gap-3",
                                            if let (Some(min_s), Some(max_s)) = (m.min_scale, m.max_scale) {
                                                span { "Scale: {min_s}–{max_s} inst" }
                                            }
                                            if let Some(ctx) = m.ctx_length {
                                                span { "Ctx: {format_count(ctx as i64)}" }
                                            }
                                        }
                                        if m.user_total_requests > 0 {
                                            span { "Usage: {m.user_total_requests} reqs ({format_count(m.user_total_tokens)} tok)" }
                                        }
                                    }

                                    // Rate Limits Progress on this Model (if any)
                                    if !m.rate_limits.is_empty() {
                                        div {
                                            class: "flex flex-col gap-1.5 p-2 rounded-lg",
                                            style: "background: rgba(0, 0, 0, 0.15); border: 1px solid var(--glow-border);",
                                            span { class: "text-xs font-bold text-dim", "Active Rate Limits:" }
                                            for rl in m.rate_limits.iter() {
                                                {
                                                    let pct = if rl.limit_value > 0 {
                                                        ((rl.used as f64) / (rl.limit_value as f64) * 100.0).min(100.0) as u32
                                                    } else {
                                                        0
                                                    };
                                                    let unit_str = match rl.unit.as_str() {
                                                        "requests" => "reqs",
                                                        "prompt_tokens" => "in-tokens",
                                                        "completion_tokens" => "out-tokens",
                                                        _ => "tokens",
                                                    };
                                                    rsx! {
                                                        div {
                                                            key: "{rl.id}",
                                                            class: "flex flex-col gap-1",
                                                            div {
                                                                class: "flex items-center justify-between text-xs font-mono",
                                                                span { "{rl.time_window}: {format_count(rl.used)} / {format_count(rl.limit_value)} {unit_str}" }
                                                                span {
                                                                    class: if pct > 90 { "text-danger font-bold" } else if pct > 70 { "text-warning" } else { "text-success" },
                                                                    "{pct}%"
                                                                }
                                                            }
                                                            div {
                                                                style: "height: 4px; background: var(--glass-bg); border-radius: 2px; overflow: hidden;",
                                                                div {
                                                                    style: format!("height: 100%; width: {}%; background: {};",
                                                                        pct,
                                                                        if pct > 90 { "var(--glow-danger, #ff4b72)" } else if pct > 70 { "var(--glow-warning, #f59e0b)" } else { "var(--glow-primary, #00f2fe)" }
                                                                    )
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // Card Footer
                                    div {
                                        class: "model-card-footer",
                                        div {
                                            if is_idle {
                                                span { class: "text-xs text-warning flex items-center gap-1",
                                                    IconClock { size: 13 }
                                                    "Will boot container on demand (this will take a while)"
                                                }
                                            } else if is_running {
                                                span { class: "text-xs text-success flex items-center gap-1",
                                                    IconBolt { size: 13 }
                                                    "Instant warm container"
                                                }
                                            } else {
                                                span { class: "text-xs text-primary flex items-center gap-1",
                                                    IconGlobe { size: 13 }
                                                    "Cloud provider upstream"
                                                }
                                            }
                                        }

                                        button {
                                            class: "glass-btn btn-primary btn-sm",
                                            onclick: move |_| {
                                                active_tab.set(ActiveTab::Chat);
                                            },
                                            IconChat { size: 14 }
                                            span { "Chat" }
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
