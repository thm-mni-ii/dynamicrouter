use crate::api::{delete_admin_instance, fetch_admin_instances};
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard};
use crate::models::InstanceSummary;
use crate::state::Toast;
use dioxus::prelude::*;

fn load_instances_action(
    mut is_loading: Signal<bool>,
    mut instances: Signal<Vec<InstanceSummary>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_instances().await {
            Ok(list) => instances.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load instances: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminInstancesView(toasts: Signal<Vec<Toast>>) -> Element {
    let instances = use_signal(Vec::<InstanceSummary>::new);
    let is_loading = use_signal(|| false);

    let _ = use_resource(move || async move {
        load_instances_action(is_loading, instances, toasts);
    });

    let on_kill_instance = move |id: String| {
        spawn(async move {
            match delete_admin_instance(&id).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Instance terminated.".to_string(),
                        is_error: false,
                    });
                    load_instances_action(is_loading, instances, toasts);
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
                            IconMonitor { size: 20 }
                            span { "Active Worker Instances" }
                        }
                        p { class: "card-description", "Live monitoring of spawned container instances currently hosting models in the pool." }
                    }
                    button {
                        class: "glass-btn btn-secondary btn-sm",
                        onclick: move |_| load_instances_action(is_loading, instances, toasts),
                        IconRefresh {
                            size: 14,
                            class: if *is_loading.read() { "animate-spin" } else { "" }
                        }
                        span { if *is_loading.read() { "Refreshing..." } else { "Refresh Instances" } }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Instance ID" }
                                th { "Model Served" }
                                th { "Status" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if instances.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 4,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading instances..." } else { "No active worker instances currently spawned (scale down to 0 idle state)." }
                                    }
                                }
                            }
                            for inst in instances.read().iter() {
                                tr {
                                    key: "{inst.id}",
                                    td { span { class: "font-mono font-bold text-sm", "{inst.id}" } }
                                    td { span { class: "text-sm", "{inst.model}" } }
                                    td {
                                        GlassBadge {
                                            variant: "glow",
                                            pulse: true,
                                            "Online / Serving"
                                        }
                                    }
                                    td {
                                        class: "text-right",
                                        button {
                                            class: "glass-btn btn-danger btn-sm",
                                            onclick: {
                                                let id = inst.id.clone();
                                                move |_| on_kill_instance(id.clone())
                                            },
                                            "Terminate"
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
