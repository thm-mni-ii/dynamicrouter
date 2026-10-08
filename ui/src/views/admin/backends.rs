use crate::api::{
    delete_admin_backend, delete_admin_backend_model, fetch_admin_backend_models,
    fetch_admin_backends, fetch_v1_models, save_admin_backend, save_admin_backend_model,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::{BackendModelRecord, BackendRecord, BackendUseFor};
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};

fn load_backends_action(
    mut is_loading: Signal<bool>,
    mut backends: Signal<Vec<BackendRecord>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_backends().await {
            Ok(list) => backends.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load backends: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminBackendsView(toasts: Signal<Vec<Toast>>) -> Element {
    let backends = use_signal(Vec::<BackendRecord>::new);
    let mut model_suggestions = use_signal(Vec::<String>::new);
    let is_loading = use_signal(|| false);
    let mut editing_backend = use_signal(|| Option::<(BackendRecord, bool)>::None);
    let mut selected_backend_for_models = use_signal(|| Option::<String>::None);
    let mut backend_models = use_signal(Vec::<BackendModelRecord>::new);

    // Form inputs for Backend
    let mut form_id = use_signal(String::new);
    let mut form_type = use_signal(|| "vllm".to_string());
    let mut form_image = use_signal(String::new);
    let mut form_quants = use_signal(Vec::<String>::new);

    // Form inputs for Model Config
    let mut editing_model_cfg = use_signal(|| Option::<(BackendModelRecord, bool)>::None);
    let mut form_model_name = use_signal(Vec::<String>::new);
    let mut form_model_args = use_signal(Vec::<String>::new);
    let mut form_model_ctx = use_signal(String::new);

    let quant_suggestions = vec![
        "AWQ".to_string(),
        "GPTQ".to_string(),
        "BitsAndBytes".to_string(),
        "GGUF".to_string(),
        "fp8".to_string(),
        "int8".to_string(),
        "int4".to_string(),
        "bf16".to_string(),
        "fp16".to_string(),
        "none".to_string(),
    ];

    let cli_arg_suggestions = vec![
        "--gpu-memory-utilization 0.95".to_string(),
        "--max-model-len 8192".to_string(),
        "--trust-remote-code".to_string(),
        "--enforce-eager".to_string(),
        "--dtype bfloat16".to_string(),
        "--tensor-parallel-size 1".to_string(),
        "--disable-log-stats".to_string(),
    ];

    let _ = use_resource(move || async move {
        load_backends_action(is_loading, backends, toasts);

        if let Ok(models) = fetch_v1_models().await {
            let mut list: Vec<String> = models.into_iter().map(|m| m.id).collect();
            list.sort();
            model_suggestions.set(list);
        }
    });

    let open_create_backend = move |_| {
        form_id.set(String::new());
        form_type.set("vllm".to_string());
        form_image.set("vllm/vllm-openai:latest".to_string());
        form_quants.set(Vec::new());

        editing_backend.set(Some((
            BackendRecord {
                id: String::new(),
                backend_type: "vllm".to_string(),
                image: "vllm/vllm-openai:latest".to_string(),
                use_for: BackendUseFor(None),
            },
            true,
        )));
    };

    let save_backend = move |_| {
        let (_, is_new) = match editing_backend.read().clone() {
            Some(b) => b,
            None => return,
        };

        let id = form_id.read().trim().to_string();
        let backend_type = form_type.read().trim().to_string();
        let image = form_image.read().trim().to_string();
        if id.is_empty() || image.is_empty() {
            return;
        }

        let quants = if form_quants.read().is_empty() {
            None
        } else {
            Some(form_quants.read().iter().cloned().collect::<HashSet<_>>())
        };

        let payload = BackendRecord {
            id,
            backend_type,
            image,
            use_for: BackendUseFor(quants),
        };

        spawn(async move {
            match save_admin_backend(&payload, is_new).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Backend configuration saved.".to_string(),
                        is_error: false,
                    });
                    editing_backend.set(None);
                    load_backends_action(is_loading, backends, toasts);
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

    let on_delete_backend = move |id: String| {
        spawn(async move {
            match delete_admin_backend(&id).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Backend deleted successfully.".to_string(),
                        is_error: false,
                    });
                    load_backends_action(is_loading, backends, toasts);
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

    let save_model_cfg = move |_| {
        let (existing_cfg, is_new) = match editing_model_cfg.read().clone() {
            Some(pair) => pair,
            None => return,
        };

        let b_id = selected_backend_for_models.read().clone().unwrap_or_default();
        let model = form_model_name.read().first().cloned().unwrap_or_default().trim().to_string();
        if model.is_empty() || b_id.is_empty() {
            return;
        }

        let args = form_model_args.read().clone();
        let ctx = form_model_ctx.read().trim().parse::<u64>().ok();
        let env = if is_new {
            HashMap::new()
        } else {
            existing_cfg.environment_variables
        };

        let payload = BackendModelRecord {
            backend_id: b_id.clone(),
            model,
            arguments: args,
            environment_variables: env,
            ctx_length: ctx,
        };

        spawn(async move {
            match save_admin_backend_model(&payload, is_new).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: if is_new {
                            "Backend model arguments saved.".to_string()
                        } else {
                            "Backend model arguments updated.".to_string()
                        },
                        is_error: false,
                    });
                    editing_model_cfg.set(None);
                    if let Ok(list) = fetch_admin_backend_models(&b_id).await {
                        backend_models.set(list);
                    }
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

    let delete_model_cfg = move |model: String| {
        let b_id = selected_backend_for_models.read().clone().unwrap_or_default();
        spawn(async move {
            let _ = delete_admin_backend_model(&b_id, &model).await;
            if let Ok(list) = fetch_admin_backend_models(&b_id).await {
                backend_models.set(list);
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
                            IconServer { size: 20 }
                            span { "Backend Runner Engines" }
                        }
                        p { class: "card-description", "Define containerized inference engines (vLLM, SGLang, TGI, llama.cpp) and per-model runtime parameters." }
                    }
                    button {
                        class: "glass-btn btn-primary",
                        onclick: open_create_backend,
                        IconPlus { size: 16 }
                        span { "Add Backend Runner" }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Backend ID" }
                                th { "Type" }
                                th { "Container Image" }
                                th { "Quant Filter" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if backends.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 5,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading backends..." } else { "No backends registered." }
                                    }
                                }
                            }
                            for backend in backends.read().iter() {
                                tr {
                                    key: "{backend.id}",
                                    td { span { class: "font-mono font-bold text-sm", "{backend.id}" } }
                                    td { GlassBadge { variant: "glow", "{backend.backend_type}" } }
                                    td { span { class: "font-mono text-xs text-muted", "{backend.image}" } }
                                    td {
                                        if let Some(ref quants) = backend.use_for.0 {
                                            GlassBadge { "{quants.len()} types" }
                                        } else {
                                            span { class: "text-muted text-xs", "All formats" }
                                        }
                                    }
                                    td {
                                        class: "text-right",
                                        div {
                                            class: "flex items-center justify-end gap-2",
                                            button {
                                                class: "glass-btn btn-primary btn-sm",
                                                onclick: {
                                                    let id = backend.id.clone();
                                                    move |_| {
                                                        selected_backend_for_models.set(Some(id.clone()));
                                                        let b_id = id.clone();
                                                        spawn(async move {
                                                            if let Ok(list) = fetch_admin_backend_models(&b_id).await {
                                                                backend_models.set(list);
                                                            }
                                                        });
                                                    }
                                                },
                                                "Model Args"
                                            }
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let b = backend.clone();
                                                    move |_| {
                                                        form_id.set(b.id.clone());
                                                        form_type.set(b.backend_type.clone());
                                                        form_image.set(b.image.clone());
                                                        let mut q_list: Vec<String> = b.use_for
                                                            .0
                                                            .as_ref()
                                                            .map(|q| q.iter().cloned().collect())
                                                            .unwrap_or_default();
                                                        q_list.sort();
                                                        form_quants.set(q_list);

                                                        editing_backend.set(Some((b.clone(), false)));
                                                    }
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "glass-btn btn-danger btn-sm",
                                                onclick: {
                                                    let id = backend.id.clone();
                                                    move |_| on_delete_backend(id.clone())
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

            // Per-Backend Models Section
            if let Some(ref b_id) = selected_backend_for_models.read().clone() {
                GlassCard {
                    div {
                        class: "card-header",
                        div {
                            h3 {
                                class: "card-title",
                                IconSliders { size: 18 }
                                span { "Custom Model Arguments for Backend: {b_id}" }
                            }
                            p { class: "card-description", "Configure command line flags, context lengths, and environment flags for specific models on this backend." }
                        }
                        div {
                            class: "flex items-center gap-2",
                            button {
                                class: "glass-btn btn-primary btn-sm",
                                onclick: {
                                    let b_id_clone = b_id.clone();
                                    move |_| {
                                        form_model_name.set(Vec::new());
                                        form_model_args.set(Vec::new());
                                        form_model_ctx.set(String::new());
                                        editing_model_cfg.set(Some((
                                            BackendModelRecord {
                                                backend_id: b_id_clone.clone(),
                                                model: String::new(),
                                                arguments: Vec::new(),
                                                environment_variables: HashMap::new(),
                                                ctx_length: None,
                                            },
                                            true,
                                        )));
                                    }
                                },
                                IconPlus { size: 14 }
                                span { "Add Model Configuration" }
                            }
                            button {
                                class: "glass-btn btn-secondary btn-sm",
                                onclick: move |_| selected_backend_for_models.set(None),
                                "Close"
                            }
                        }
                    }

                    div {
                        class: "glass-table-container",
                        table {
                            class: "glass-table",
                            thead {
                                tr {
                                    th { "Model" }
                                    th { "CLI Arguments" }
                                    th { "Max Ctx Length" }
                                    th { class: "text-right", "Actions" }
                                }
                            }
                            tbody {
                                if backend_models.read().is_empty() {
                                    tr {
                                        td {
                                            colspan: 4,
                                            class: "text-muted",
                                            style: "text-align: center; padding: 2rem;",
                                            "No model-specific arguments overridden. Defaults will be used."
                                        }
                                    }
                                }
                                for cfg in backend_models.read().iter() {
                                    tr {
                                        key: "{cfg.model}",
                                        td { span { class: "font-mono font-bold text-sm", "{cfg.model}" } }
                                        td { span { class: "font-mono text-xs text-muted", "{cfg.arguments.join(\" \")}" } }
                                        td { span { class: "text-sm", "{cfg.ctx_length.map(|c| c.to_string()).unwrap_or(\"Default\".to_string())}" } }
                                        td {
                                            class: "text-right",
                                            div {
                                                class: "flex items-center justify-end gap-2",
                                                button {
                                                    class: "glass-btn btn-secondary btn-sm",
                                                    onclick: {
                                                        let cfg = cfg.clone();
                                                        move |_| {
                                                            form_model_name.set(vec![cfg.model.clone()]);
                                                            form_model_args.set(cfg.arguments.clone());
                                                            form_model_ctx.set(cfg.ctx_length.map(|c| c.to_string()).unwrap_or_default());
                                                            editing_model_cfg.set(Some((cfg.clone(), false)));
                                                        }
                                                    },
                                                    "Edit"
                                                }
                                                button {
                                                    class: "glass-btn btn-danger btn-sm",
                                                    onclick: {
                                                        let m = cfg.model.clone();
                                                        move |_| delete_model_cfg(m.clone())
                                                    },
                                                    "Remove"
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

            // Backend Edit Modal
            if let Some((_, is_new)) = editing_backend.read().clone() {
                GlassModal {
                    title: if is_new { "Create Backend Runner".to_string() } else { format!("Edit Backend: {}", form_id.read()) },
                    on_close: move |_| editing_backend.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Backend Identifier (e.g. vllm, sglang-cuda)" }
                            input {
                                class: "glass-input font-mono",
                                disabled: !is_new,
                                value: "{form_id.read()}",
                                oninput: move |evt| form_id.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Backend Type" }
                            select {
                                class: "glass-select",
                                value: "{form_type.read()}",
                                onchange: move |evt| form_type.set(evt.value()),
                                option { value: "vllm", "vLLM (High Throughput LLM Inference)" }
                                option { value: "sglang", "SGLang (Structured / Fast Inference)" }
                                option { value: "tgi", "Text Generation Inference (HuggingFace TGI)" }
                                option { value: "llama_cpp", "llama.cpp (GGUF / CPU+GPU)" }
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Container Image" }
                            input {
                                class: "glass-input font-mono",
                                placeholder: "vllm/vllm-openai:latest",
                                value: "{form_image.read()}",
                                oninput: move |evt| form_image.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Quantization Types to Use For (optional)" }
                            ChipInput {
                                values: form_quants,
                                suggestions: quant_suggestions.clone(),
                                placeholder: "Add quantization type (AWQ, GPTQ, GGUF, fp8)...",
                            }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| editing_backend.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_backend,
                                "Save Backend"
                            }
                        }
                    }
                }
            }

            // Model Cfg Modal
            if let Some((_, is_new)) = editing_model_cfg.read().clone() {
                GlassModal {
                    title: if is_new {
                        "Add Backend Model Arguments".to_string()
                    } else {
                        format!("Edit Model Arguments: {}", form_model_name.read().first().cloned().unwrap_or_default())
                    },
                    on_close: move |_| editing_model_cfg.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Model ID" }
                            ChipInput {
                                values: form_model_name,
                                suggestions: model_suggestions.read().clone(),
                                max_items: Some(1),
                                disabled: !is_new,
                                placeholder: "Select or enter model ID...",
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Command Line Arguments" }
                            ChipInput {
                                values: form_model_args,
                                suggestions: cli_arg_suggestions.clone(),
                                placeholder: "e.g. --gpu-memory-utilization 0.95...",
                            }
                            span { class: "form-hint", "Add individual arguments as chips. Comma or Enter to add." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Context Length (Optional)" }
                            input {
                                r#type: "number",
                                class: "glass-input",
                                placeholder: "8192",
                                value: "{form_model_ctx.read()}",
                                oninput: move |evt| form_model_ctx.set(evt.value()),
                            }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| editing_model_cfg.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_model_cfg,
                                "Save Model Config"
                            }
                        }
                    }
                }
            }
        }
    }
}
