use crate::api::fetch_user_metrics;
use crate::components::icons::*;
use crate::components::GlassCard;
use crate::models::UserMetric;
use dioxus::prelude::*;

fn load_metrics_action(mut is_loading: Signal<bool>, mut metrics: Signal<Vec<UserMetric>>) {
    is_loading.set(true);
    spawn(async move {
        if let Ok(list) = fetch_user_metrics().await {
            metrics.set(list);
        }
        is_loading.set(false);
    });
}

#[component]
pub fn UserMetricsView() -> Element {
    let metrics = use_signal(Vec::<UserMetric>::new);
    let is_loading = use_signal(|| false);

    let _ = use_resource(move || async move {
        load_metrics_action(is_loading, metrics);
    });

    let total_reqs: i64 = metrics.read().iter().map(|m| m.total_requests).sum();

    rsx! {
        div {
            class: "flex flex-col gap-4",
            
            // KPI Summary Cards
            div {
                class: "kpi-grid",
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Total Prompts" }
                    span { class: "kpi-value", "{total_reqs}" }
                    span { class: "kpi-subtext", "Lifetime requests submitted by your account" }
                }
                GlassCard {
                    class: "kpi-card",
                    span { class: "kpi-title", "Active Models Used" }
                    span { class: "kpi-value", "{metrics.read().len()}" }
                    span { class: "kpi-subtext", "Distinct models with activity" }
                }
            }

            // Breakdown by Model
            GlassCard {
                div {
                    class: "card-header",
                    div {
                        h3 {
                            class: "card-title",
                            IconMetrics { size: 18 }
                            span { "Usage by Model" }
                        }
                        p { class: "card-description", "Breakdown of prompt volume and response latencies per model." }
                    }
                    button {
                        class: "glass-btn btn-secondary btn-sm",
                        onclick: move |_| load_metrics_action(is_loading, metrics),
                        IconRefresh {
                            size: 14,
                            class: if *is_loading.read() { "animate-spin" } else { "" }
                        }
                        span { if *is_loading.read() { "Refreshing..." } else { "Refresh" } }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "User / Subject" }
                                th { "Total Requests" }
                                th { "Avg Latency (s)" }
                            }
                        }
                        tbody {
                            if metrics.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 3,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading metrics..." } else { "No metrics recorded yet." }
                                    }
                                }
                            }
                            for item in metrics.read().iter() {
                                tr {
                                    key: "{item.user}",
                                    td {
                                        span { class: "font-mono text-sm font-bold", "{item.user}" }
                                    }
                                    td {
                                        span { class: "text-sm", "{item.total_requests}" }
                                    }
                                    td {
                                        span { class: "text-sm",
                                            if let Some(avg) = item.avg_duration_secs {
                                                "{avg:.3}s"
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
