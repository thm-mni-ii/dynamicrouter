use crate::api::{fetch_my_rate_limits, fetch_user_metrics};
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard};
use crate::models::{RateLimitUsage, UserMetric};
use crate::views::admin::format_count;
use dioxus::prelude::*;

fn load_metrics_action(
    mut is_loading: Signal<bool>,
    mut metrics: Signal<Vec<UserMetric>>,
    mut rate_limits: Signal<Vec<RateLimitUsage>>,
) {
    is_loading.set(true);
    spawn(async move {
        if let Ok(list) = fetch_user_metrics().await {
            metrics.set(list);
        }
        if let Ok(rl_list) = fetch_my_rate_limits().await {
            rate_limits.set(rl_list);
        }
        is_loading.set(false);
    });
}

#[component]
pub fn UserMetricsView() -> Element {
    let metrics = use_signal(Vec::<UserMetric>::new);
    let rate_limits = use_signal(Vec::<RateLimitUsage>::new);
    let is_loading = use_signal(|| false);

    let _ = use_resource(move || async move {
        load_metrics_action(is_loading, metrics, rate_limits);
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
                    span { class: "kpi-title", "Active Quotas & Limits" }
                    span { class: "kpi-value", "{rate_limits.read().len()}" }
                    span { class: "kpi-subtext", "Assigned compute rate limits & token budgets" }
                }
            }

            // Rate Limits & Quotas Section
            if !rate_limits.read().is_empty() {
                GlassCard {
                    div {
                        class: "card-header",
                        div {
                            h3 {
                                class: "card-title",
                                IconGauge { size: 18 }
                                span { "Your Compute Quotas & Rate Limits" }
                            }
                            p { class: "card-description", "Real-time tracking of your token quotas and rate limits across active time windows." }
                        }
                    }

                    div {
                        class: "grid grid-cols-1 md:grid-cols-2 gap-3",
                        for rl in rate_limits.read().iter() {
                            {
                                let pct = if rl.limit_value > 0 {
                                    ((rl.used as f64) / (rl.limit_value as f64) * 100.0).min(100.0) as u32
                                } else {
                                    0
                                };
                                let unit_display = match rl.unit.as_str() {
                                    "requests" => "requests",
                                    "prompt_tokens" => "input tokens",
                                    "completion_tokens" => "output tokens",
                                    _ => "tokens",
                                };
                                let model_title = if let Some(ref m) = rl.model {
                                    format!("Model: {m}")
                                } else {
                                    "All Models Combined".to_string()
                                };
                                let window_title = match rl.time_window.as_str() {
                                    "minute" => "Per Minute",
                                    "hour" => "Per Hour",
                                    "day" => "Per Day",
                                    "week" => "Per Week",
                                    "month" => "Per Month",
                                    _ => "Permanent",
                                };
                                rsx! {
                                    div {
                                        key: "{rl.id}",
                                        class: "p-3 flex flex-col gap-2",
                                        style: "background: var(--glow-surface); border: 1px solid var(--glow-border); border-radius: 12px;",
                                        
                                        div {
                                            class: "flex items-center justify-between",
                                            div {
                                                class: "flex items-center gap-2",
                                                span { class: "text-sm font-bold", "{model_title}" }
                                            }
                                            GlassBadge {
                                                variant: "primary",
                                                "{window_title}"
                                            }
                                        }

                                        div {
                                            class: "flex items-center justify-between text-xs font-mono text-muted",
                                            span { "{format_count(rl.used)} / {format_count(rl.limit_value)} {unit_display}" }
                                            span { class: if pct > 90 { "text-danger font-bold" } else if pct > 70 { "text-warning" } else { "text-success" }, "{pct}% used" }
                                        }

                                        div {
                                            style: "height: 8px; background: var(--glass-bg); border-radius: 4px; overflow: hidden; border: 1px solid var(--glow-border);",
                                            div {
                                                style: format!("height: 100%; width: {}%; background: {}; border-radius: 4px; transition: width 0.3s ease;",
                                                    pct,
                                                    if pct > 90 { "var(--glow-danger, #ff4b72)" } else if pct > 70 { "var(--glow-warning, #f59e0b)" } else { "var(--glow-primary, #00f2fe)" }
                                                )
                                            }
                                        }

                                        div {
                                            class: "flex items-center justify-between text-xs text-dim mt-1",
                                            span { "Remaining: {format_count(rl.remaining)} {unit_display}" }
                                            if let Some(reset_sec) = rl.reset_in_seconds {
                                                if reset_sec > 0 {
                                                    span { class: "font-mono text-xs", "Resets in: {reset_sec}s" }
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
                        onclick: move |_| load_metrics_action(is_loading, metrics, rate_limits),
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
