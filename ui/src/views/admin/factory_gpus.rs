use crate::api::{
    delete_admin_factory_gpu, fetch_admin_factory_gpus, fetch_v1_models, save_admin_factory_gpu,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::FactoryGpuRecord;
use crate::state::Toast;
use dioxus::prelude::*;

fn load_gpus_action(
    mut is_loading: Signal<bool>,
    mut gpus: Signal<Vec<FactoryGpuRecord>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_factory_gpus().await {
            Ok(list) => gpus.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load GPU specs: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminFactoryGpusView(toasts: Signal<Vec<Toast>>) -> Element {
    let gpus = use_signal(Vec::<FactoryGpuRecord>::new);
    let mut model_suggestions = use_signal(Vec::<String>::new);
    let is_loading = use_signal(|| false);
    let mut show_create_modal = use_signal(|| false);

    // Form inputs
    let mut form_factory = use_signal(|| "runpod".to_string());
    let mut form_id = use_signal(String::new);
    let mut form_memory = use_signal(|| 24i64);
    let mut form_price = use_signal(|| 0.45f64);
    let mut form_max_count = use_signal(|| 8i64);
    let mut form_model = use_signal(Vec::<String>::new);

    let _ = use_resource(move || async move {
        load_gpus_action(is_loading, gpus, toasts);

        if let Ok(models) = fetch_v1_models().await {
            let mut list: Vec<String> = models.into_iter().map(|m| m.id).collect();
            list.sort();
            model_suggestions.set(list);
        }
    });

    let save_gpu = move |_| {
        let factory_type = form_factory.read().trim().to_string();
        let id = form_id.read().trim().to_string();
        if id.is_empty() {
            return;
        }

        let model_val = form_model.read().first().cloned().unwrap_or_default().trim().to_string();

        let payload = FactoryGpuRecord {
            factory_type,
            id,
            memory: *form_memory.read(),
            price: *form_price.read(),
            max_count: *form_max_count.read(),
            model: model_val,
        };

        spawn(async move {
            match save_admin_factory_gpu(&payload).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Factory GPU registered.".to_string(),
                        is_error: false,
                    });
                    show_create_modal.set(false);
                    load_gpus_action(is_loading, gpus, toasts);
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

    let on_delete_gpu = move |f_type: String, id: String, model: String| {
        spawn(async move {
            match delete_admin_factory_gpu(&f_type, &id, &model).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Factory GPU deleted.".to_string(),
                        is_error: false,
                    });
                    load_gpus_action(is_loading, gpus, toasts);
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
                            IconGpu { size: 20 }
                            span { "Cloud & Cluster GPU Specifications" }
                        }
                        p { class: "card-description", "Configure available hardware resources across spawner factories (RunPod, Kubernetes, local clusters)." }
                    }
                    button {
                        class: "glass-btn btn-primary",
                        onclick: move |_| {
                            form_factory.set("runpod".to_string());
                            form_id.set(String::new());
                            form_memory.set(24);
                            form_price.set(0.45);
                            form_max_count.set(8);
                            form_model.set(Vec::new());
                            show_create_modal.set(true);
                        },
                        IconPlus { size: 16 }
                        span { "Add GPU Spec" }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Factory" }
                                th { "GPU ID" }
                                th { "VRAM (GB)" }
                                th { "Price / Hr" }
                                th { "Max Limit" }
                                th { "Bound Model" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if gpus.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 7,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading GPU specs..." } else { "No factory GPU types registered." }
                                    }
                                }
                            }
                            for gpu in gpus.read().iter() {
                                tr {
                                    key: "{gpu.factory_type}-{gpu.id}-{gpu.model}",
                                    td { GlassBadge { variant: "glow", "{gpu.factory_type}" } }
                                    td { span { class: "font-mono font-bold text-sm", "{gpu.id}" } }
                                    td { span { class: "text-sm", "{gpu.memory} GB" } }
                                    td { span { class: "text-sm font-mono", "${gpu.price:.2}/hr" } }
                                    td { span { class: "text-sm", "{gpu.max_count}" } }
                                    td {
                                        if !gpu.model.is_empty() {
                                            span { class: "font-mono text-xs", "{gpu.model}" }
                                        } else {
                                            span { class: "text-muted text-xs", "All Models" }
                                        }
                                    }
                                    td {
                                        class: "text-right",
                                        button {
                                            class: "glass-btn btn-danger btn-sm",
                                            onclick: {
                                                let f = gpu.factory_type.clone();
                                                let id = gpu.id.clone();
                                                let m = gpu.model.clone();
                                                move |_| on_delete_gpu(f.clone(), id.clone(), m.clone())
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

            // Create Modal
            if *show_create_modal.read() {
                GlassModal {
                    title: "Register Factory GPU".to_string(),
                    on_close: move |_| show_create_modal.set(false),
                    div {
                        class: "flex flex-col gap-3",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Factory Provider" }
                            select {
                                class: "glass-select",
                                value: "{form_factory.read()}",
                                onchange: move |evt| form_factory.set(evt.value()),
                                option { value: "runpod", "RunPod Serverless / Pods" }
                                option { value: "kubernetes", "Kubernetes Cluster Nodes" }
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "GPU Model ID (e.g. NVIDIA-A100-80GB, RTX-4090)" }
                            input {
                                class: "glass-input font-mono",
                                value: "{form_id.read()}",
                                oninput: move |evt| form_id.set(evt.value()),
                            }
                        }

                        div {
                            style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 0.75rem;",
                            div {
                                class: "form-group",
                                label { class: "form-label", "VRAM (GB)" }
                                input {
                                    r#type: "number",
                                    class: "glass-input",
                                    value: "{form_memory.read()}",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<i64>() { form_memory.set(v); }
                                    }
                                }
                            }
                            div {
                                class: "form-group",
                                label { class: "form-label", "Price / Hr ($)" }
                                input {
                                    r#type: "number",
                                    step: "0.01",
                                    class: "glass-input",
                                    value: "{form_price.read()}",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<f64>() { form_price.set(v); }
                                    }
                                }
                            }
                            div {
                                class: "form-group",
                                label { class: "form-label", "Max Count" }
                                input {
                                    r#type: "number",
                                    class: "glass-input",
                                    value: "{form_max_count.read()}",
                                    oninput: move |evt| {
                                        if let Ok(v) = evt.value().parse::<i64>() { form_max_count.set(v); }
                                    }
                                }
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Target Model Restriction (Optional, empty for general use)" }
                            ChipInput {
                                values: form_model,
                                suggestions: model_suggestions.read().clone(),
                                max_items: Some(1),
                                placeholder: "Select model restriction...",
                            }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| show_create_modal.set(false),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_gpu,
                                "Register GPU"
                            }
                        }
                    }
                }
            }
        }
    }
}
