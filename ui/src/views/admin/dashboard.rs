use crate::api::{
    fetch_admin_metrics_summary, fetch_admin_model_metrics, fetch_admin_timeseries,
    fetch_admin_user_metrics,
};
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard};
use crate::models::{MetricsSummary, ModelMetric, TimeseriesPoint, UserMetric};
use dioxus::prelude::*;

fn load_dashboard_action(
    mut is_loading: Signal<bool>,
    mut summary: Signal<MetricsSummary>,
    mut model_metrics: Signal<Vec<ModelMetric>>,
    mut user_metrics: Signal<Vec<UserMetric>>,
    mut timeseries: Signal<Vec<TimeseriesPoint>>,
) {
    is_loading.set(true);
    spawn(async move {
        if let Ok(s) = fetch_admin_metrics_summary().await {
            summary.set(s);
        }
        if let Ok(m) = fetch_admin_model_metrics().await {
            model_metrics.set(m);
        }
        if let Ok(u) = fetch_admin_user_metrics().await {
            user_metrics.set(u);
        }
        if let Ok(t) = fetch_admin_timeseries(48, 1800).await {
            timeseries.set(t);
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminDashboardView() -> Element {
    let summary = use_signal(MetricsSummary::default);
    let model_metrics = use_signal(Vec::<ModelMetric>::new);
    let user_metrics = use_signal(Vec::<UserMetric>::new);
    let timeseries = use_signal(Vec::<TimeseriesPoint>::new);
    let is_loading = use_signal(|| false);

    let _ = use_resource(move || async move {
        load_dashboard_action(is_loading, summary, model_metrics, user_metrics, timeseries);
    });

    let s = summary.read().clone();
    let max_chart_count = timeseries
        .read()
        .iter()
        .map(|p| p.count)
        .max()
        .unwrap_or(1)
        .max(1);

    rsx! {
        div {
            class: "flex flex-col gap-4",
            
            // Header
            div {
                class: "flex items-center justify-between",
                div {
                    h2 {
                        class: "card-title",
                        IconDashboard { size: 20 }
                        span { "Central Cluster Metrics" }
                    }
                    p { class: "card-description", "Real-time statistics across all dynamic model backends, instances, and upstreams." }
                }
                button {
                    class: "glass-btn btn-primary btn-sm",
                    onclick: move |_| load_dashboard_action(is_loading, summary, model_metrics, user_metrics, timeseries),
                    IconRefresh {
                        size: 14,
                        class: if *is_loading.read() { "animate-spin" } else { "" }
                    }
                    span { if *is_loading.read() { "Updating..." } else { "Refresh Data" } }
                }
            }

            // Top KPI Cards
            div {
                class: "kpi-grid",
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Total Requests" }
                    span { class: "kpi-value", "{s.total_requests}" }
                    span { class: "kpi-subtext", "Processed since inception" }
                }
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Pending / In-Flight" }
                    span { class: "kpi-value", "{s.pending_requests}" }
                    span { class: "kpi-subtext", "Active queues across models" }
                }
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Active Instances" }
                    span { class: "kpi-value", "{s.active_instances}" }
                    span { class: "kpi-subtext", "Running spawner containers" }
                }
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Avg Latency" }
                    span {
                        class: "kpi-value",
                        if let Some(avg) = s.avg_duration_secs {
                            "{avg:.2}s"
                        } else {
                            "—"
                        }
                    }
                    span {
                        class: "kpi-subtext",
                        "P50: "
                        if let Some(p50) = s.p50_duration_secs { "{p50:.2}s" } else { "—" }
                        " | P99: "
                        if let Some(p99) = s.p99_duration_secs { "{p99:.2}s" } else { "—" }
                    }
                }
            }

            // Timeseries Visualizer
            GlassCard {
                div {
                    class: "card-header",
                    div {
                        h3 {
                            class: "card-title",
                            IconMetrics { size: 18 }
                            span { "Traffic Activity (Last 24 Hours)" }
                        }
                        p { class: "card-description", "Histogram buckets of request volume over time." }
                    }
                }

                if timeseries.read().is_empty() {
                    div {
                        class: "text-muted",
                        style: "padding: 2rem; text-align: center;",
                        "No timeseries data collected in this window."
                    }
                } else {
                    div {
                        class: "timeseries-chart",
                        for point in timeseries.read().iter() {
                            div {
                                key: "{point.timestamp}",
                                class: "chart-bar",
                                style: "height: {((point.count as f64 / max_chart_count as f64) * 100.0).max(4.0)}%;",
                                title: "Time: {point.timestamp} | Requests: {point.count}",
                            }
                        }
                    }
                }
            }

            // Two Column Grid: Models & Users Breakdown
            div {
                style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(400px, 1fr)); gap: 1.5rem;",
                
                // Models Table
                GlassCard {
                    div {
                        class: "card-header",
                        h3 {
                            class: "card-title",
                            IconBot { size: 18 }
                            span { "Model Traffic Breakdown" }
                        }
                    }
                    div {
                        class: "glass-table-container",
                        table {
                            class: "glass-table",
                            thead {
                                tr {
                                    th { "Model" }
                                    th { "Requests" }
                                    th { "Instances" }
                                    th { "Avg (s)" }
                                }
                            }
                            tbody {
                                if model_metrics.read().is_empty() {
                                    tr {
                                        td { colspan: 4, class: "text-muted text-center", "No model traffic recorded." }
                                    }
                                }
                                for item in model_metrics.read().iter() {
                                    tr {
                                        key: "{item.model}",
                                        td { span { class: "font-bold text-sm", "{item.model}" } }
                                        td { "{item.total_requests}" }
                                        td {
                                            if item.active_instances > 0 {
                                                GlassBadge { variant: "glow", "{item.active_instances} active" }
                                            } else {
                                                span { class: "text-muted text-xs", "0" }
                                            }
                                        }
                                        td {
                                            if let Some(avg) = item.avg_duration_secs {
                                                "{avg:.2}s"
                                            } else {
                                                "—"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // Users Table
                GlassCard {
                    div {
                        class: "card-header",
                        h3 {
                            class: "card-title",
                            IconUsers { size: 18 }
                            span { "User Traffic Breakdown" }
                        }
                    }
                    div {
                        class: "glass-table-container",
                        table {
                            class: "glass-table",
                            thead {
                                tr {
                                    th { "User" }
                                    th { "Requests" }
                                    th { "Avg Latency" }
                                }
                            }
                            tbody {
                                if user_metrics.read().is_empty() {
                                    tr {
                                        td { colspan: 3, class: "text-muted text-center", "No user activity recorded." }
                                    }
                                }
                                for item in user_metrics.read().iter() {
                                    tr {
                                        key: "{item.user}",
                                        td { span { class: "font-mono font-bold text-sm", "{item.user}" } }
                                        td { "{item.total_requests}" }
                                        td {
                                            if let Some(avg) = item.avg_duration_secs {
                                                "{avg:.2}s"
                                            } else {
                                                "—"
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
