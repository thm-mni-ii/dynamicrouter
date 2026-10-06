use crate::api::{
    delete_admin_model, fetch_admin_backends, fetch_admin_models, fetch_admin_openai_apis,
    fetch_admin_users, save_admin_model,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::{
    AdminModelConfig, ModelHandlerConfig, OpenAiHandlerConf, SpawnerConfig,
};
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::HashSet;

fn load_models_action(
    mut is_loading: Signal<bool>,
    mut models: Signal<Vec<AdminModelConfig>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_models().await {
            Ok(list) => models.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to fetch models: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminModelsView(toasts: Signal<Vec<Toast>>) -> Element {
    let models = use_signal(Vec::<AdminModelConfig>::new);
    let mut user_suggestions = use_signal(Vec::<String>::new);
    let mut backend_suggestions = use_signal(Vec::<String>::new);
    let mut openai_api_suggestions = use_signal(Vec::<String>::new);
    let is_loading = use_signal(|| false);
    let mut editing_model = use_signal(|| Option::<(AdminModelConfig, bool)>::None);

    // Edit form fields
    let mut form_id = use_signal(String::new);
    let mut form_name = use_signal(String::new);
    let mut form_timeout = use_signal(String::new);
    let mut form_hidden = use_signal(|| false);
    let mut form_users = use_signal(Vec::<String>::new);
    let mut form_handler_type = use_signal(|| "spawner".to_string());
    
    // Spawner fields
    let mut form_backends = use_signal(Vec::<String>::new);
    let mut form_min_scale = use_signal(|| 0u32);
    let mut form_max_scale = use_signal(|| 1u32);
    let mut form_queue_threshold = use_signal(|| 1u32);

    // OpenAI fields
    let mut form_openai_api = use_signal(Vec::<String>::new);
    let mut form_openai_url = use_signal(String::new);
    let mut form_openai_token = use_signal(String::new);
    let mut form_openai_model = use_signal(String::new);

    let _ = use_resource(move || async move {
        load_models_action(is_loading, models, toasts);

        if let Ok(users) = fetch_admin_users().await {
            let mut list: Vec<String> = users.into_iter().map(|u| u.username).collect();
            list.sort();
            user_suggestions.set(list);
        }

        let mut backends_list = vec![
            "vllm".to_string(),
            "sglang".to_string(),
            "tgi".to_string(),
            "llama_cpp".to_string(),
        ];
        if let Ok(backends) = fetch_admin_backends().await {
            for b in backends {
                if !backends_list.contains(&b.id) {
                    backends_list.push(b.id);
                }
            }
        }
        backends_list.sort();
        backend_suggestions.set(backends_list);

        if let Ok(apis) = fetch_admin_openai_apis().await {
            let mut list: Vec<String> = apis.into_iter().map(|a| a.id).collect();
            list.sort();
            openai_api_suggestions.set(list);
        }
    });

    let open_create_modal = move |_| {
        form_id.set(String::new());
        form_name.set(String::new());
        form_timeout.set(String::new());
        form_hidden.set(false);
        form_users.set(Vec::new());
        form_handler_type.set("spawner".to_string());
        form_backends.set(Vec::new());
        form_min_scale.set(0);
        form_max_scale.set(1);
        form_queue_threshold.set(1);
        form_openai_api.set(Vec::new());
        form_openai_url.set(String::new());
        form_openai_token.set(String::new());
        form_openai_model.set(String::new());

        editing_model.set(Some((
            AdminModelConfig {
                id: String::new(),
                name: None,
                timeout: None,
                limit_to_users: None,
                hidden: Some(false),
                handler: ModelHandlerConfig::Spawner(SpawnerConfig::default()),
            },
            true,
        )));
    };

    let save_model = move |_| {
        let (_, is_new) = match editing_model.read().clone() {
            Some(m) => m,
            None => return,
        };

        let id = form_id.read().trim().to_string();
        if id.is_empty() {
            return;
        }

        let name = if form_name.read().trim().is_empty() {
            None
        } else {
            Some(form_name.read().trim().to_string())
        };

        let timeout = form_timeout.read().trim().parse::<u32>().ok();

        let limit_to_users = if form_users.read().is_empty() {
            None
        } else {
            Some(form_users.read().iter().cloned().collect::<HashSet<_>>())
        };

        let handler = if form_handler_type.read().as_str() == "openai" {
            let api_val = form_openai_api.read().first().cloned().filter(|s| !s.trim().is_empty());
            ModelHandlerConfig::OpenAi(OpenAiHandlerConf {
                api: api_val,
                url: if form_openai_url.read().trim().is_empty() { None } else { Some(form_openai_url.read().trim().to_string()) },
                token: if form_openai_token.read().trim().is_empty() { None } else { Some(form_openai_token.read().trim().to_string()) },
                model: if form_openai_model.read().trim().is_empty() { None } else { Some(form_openai_model.read().trim().to_string()) },
                timeout,
                headers: None,
            })
        } else {
            let backends = if form_backends.read().is_empty() {
                None
            } else {
                Some(form_backends.read().iter().cloned().collect::<HashSet<_>>())
            };
            ModelHandlerConfig::Spawner(SpawnerConfig {
                backends,
                min_scale: *form_min_scale.read(),
                max_scale: *form_max_scale.read(),
                queue_length_for_scale_up: *form_queue_threshold.read(),
            })
        };

        let payload = AdminModelConfig {
            id,
            name,
            timeout,
            limit_to_users,
            hidden: Some(*form_hidden.read()),
            handler,
        };

        spawn(async move {
            match save_admin_model(&payload, is_new).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Model saved successfully.".to_string(),
                        is_error: false,
                    });
                    editing_model.set(None);
                    load_models_action(is_loading, models, toasts);
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

    let on_delete_model = move |id: String| {
        spawn(async move {
            match delete_admin_model(&id).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Model deleted successfully.".to_string(),
                        is_error: false,
                    });
                    load_models_action(is_loading, models, toasts);
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
                            IconBot { size: 20 }
                            span { "Model Routing & Scaling" }
                        }
                        p { class: "card-description", "Configure local/cloud spawner backend pipelines and upstream OpenAI endpoint targets." }
                    }
                    div {
                        class: "flex items-center gap-2",
                        button {
                            class: "glass-btn btn-primary",
                            onclick: open_create_modal,
                            IconPlus { size: 16 }
                            span { "Add Model" }
                        }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Model ID" }
                                th { "Display Name" }
                                th { "Handler" }
                                th { "Scale (Min/Max)" }
                                th { "Timeout" }
                                th { "Visibility" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if models.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 7,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading models..." } else { "No configured models found." }
                                    }
                                }
                            }
                            for model in models.read().iter() {
                                tr {
                                    key: "{model.id}",
                                    td {
                                        span { class: "font-mono font-bold text-sm", "{model.id}" }
                                    }
                                    td {
                                        span { class: "text-sm", "{model.name.as_deref().unwrap_or(\"—\")}" }
                                    }
                                    td {
                                        match &model.handler {
                                            ModelHandlerConfig::Spawner(_) => rsx! { GlassBadge { variant: "glow", "Spawner" } },
                                            ModelHandlerConfig::OpenAi(_) => rsx! { GlassBadge { "OpenAI Upstream" } },
                                        }
                                    }
                                    td {
                                        span {
                                            class: "text-sm font-mono",
                                            match &model.handler {
                                                ModelHandlerConfig::Spawner(sp) => format!("{}/{}", sp.min_scale, sp.max_scale),
                                                ModelHandlerConfig::OpenAi(_) => "—".to_string(),
                                            }
                                        }
                                    }
                                    td {
                                        span {
                                            class: "text-sm font-mono text-muted",
                                            match model.timeout {
                                                Some(t) => format!("{t}s"),
                                                None => "Default".to_string(),
                                            }
                                        }
                                    }
                                    td {
                                        if model.hidden.unwrap_or(false) {
                                            GlassBadge { variant: "warning", "Hidden" }
                                        } else {
                                            GlassBadge { variant: "success", "Public" }
                                        }
                                    }
                                    td {
                                        class: "text-right",
                                        div {
                                            class: "flex items-center justify-end gap-2",
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let m = model.clone();
                                                    move |_| {
                                                        form_id.set(m.id.clone());
                                                        form_name.set(m.name.clone().unwrap_or_default());
                                                        form_timeout.set(m.timeout.map(|t| t.to_string()).unwrap_or_default());
                                                        form_hidden.set(m.hidden.unwrap_or(false));
                                                        let mut u_list: Vec<String> = m.limit_to_users
                                                            .as_ref()
                                                            .map(|u| u.iter().cloned().collect())
                                                            .unwrap_or_default();
                                                        u_list.sort();
                                                        form_users.set(u_list);

                                                        match &m.handler {
                                                            ModelHandlerConfig::Spawner(sp) => {
                                                                form_handler_type.set("spawner".to_string());
                                                                let mut b_list: Vec<String> = sp.backends
                                                                    .as_ref()
                                                                    .map(|b| b.iter().cloned().collect())
                                                                    .unwrap_or_default();
                                                                b_list.sort();
                                                                form_backends.set(b_list);
                                                                form_min_scale.set(sp.min_scale);
                                                                form_max_scale.set(sp.max_scale);
                                                                form_queue_threshold.set(sp.queue_length_for_scale_up);
                                                            }
                                                            ModelHandlerConfig::OpenAi(oa) => {
                                                                form_handler_type.set("openai".to_string());
                                                                form_openai_api.set(oa.api.as_ref().map(|a| vec![a.clone()]).unwrap_or_default());
                                                                form_openai_url.set(oa.url.clone().unwrap_or_default());
                                                                form_openai_token.set(oa.token.clone().unwrap_or_default());
                                                                form_openai_model.set(oa.model.clone().unwrap_or_default());
                                                            }
                                                        }

                                                        editing_model.set(Some((m.clone(), false)));
                                                    }
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "glass-btn btn-danger btn-sm",
                                                onclick: {
                                                    let id = model.id.clone();
                                                    move |_| on_delete_model(id.clone())
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

            // Add/Edit Model Modal
            if let Some((_, is_new)) = editing_model.read().clone() {
                GlassModal {
                    title: if is_new { "Add New Model".to_string() } else { format!("Edit Model: {}", form_id.read()) },
                    on_close: move |_| editing_model.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Model ID (e.g. meta-llama/Llama-3-8B-Instruct)" }
                            input {
                                class: "glass-input font-mono",
                                disabled: !is_new,
                                value: "{form_id.read()}",
                                oninput: move |evt| form_id.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Display Name" }
                            input {
                                class: "glass-input",
                                value: "{form_name.read()}",
                                oninput: move |evt| form_name.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Handler Engine" }
                            select {
                                class: "glass-select",
                                value: "{form_handler_type.read()}",
                                onchange: move |evt| form_handler_type.set(evt.value()),
                                option { value: "spawner", "Dynamic Spawner (GPU / Worker Pool)" }
                                option { value: "openai", "OpenAI-Compatible Upstream" }
                            }
                        }

                        // Spawner specific fields
                        if form_handler_type.read().as_str() == "spawner" {
                            div {
                                class: "flex flex-col gap-3 p-3",
                                style: "background: var(--glass-bg-subtle); border-radius: 12px; border: 1px solid var(--glass-border);",
                                
                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Allowed Backends (optional, empty for all)" }
                                    ChipInput {
                                        values: form_backends,
                                        suggestions: backend_suggestions.read().clone(),
                                        placeholder: "e.g. vllm, sglang, tgi...",
                                    }
                                }

                                div {
                                    style: "display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 0.75rem;",
                                    div {
                                        class: "form-group",
                                        label { class: "form-label", "Min Scale" }
                                        input {
                                            r#type: "number",
                                            class: "glass-input",
                                            value: "{form_min_scale.read()}",
                                            oninput: move |evt| {
                                                if let Ok(v) = evt.value().parse::<u32>() { form_min_scale.set(v); }
                                            }
                                        }
                                    }
                                    div {
                                        class: "form-group",
                                        label { class: "form-label", "Max Scale" }
                                        input {
                                            r#type: "number",
                                            class: "glass-input",
                                            value: "{form_max_scale.read()}",
                                            oninput: move |evt| {
                                                if let Ok(v) = evt.value().parse::<u32>() { form_max_scale.set(v); }
                                            }
                                        }
                                    }
                                    div {
                                        class: "form-group",
                                        label { class: "form-label", "Queue Threshold" }
                                        input {
                                            r#type: "number",
                                            class: "glass-input",
                                            value: "{form_queue_threshold.read()}",
                                            oninput: move |evt| {
                                                if let Ok(v) = evt.value().parse::<u32>() { form_queue_threshold.set(v); }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // OpenAI Upstream specific fields
                        if form_handler_type.read().as_str() == "openai" {
                            div {
                                class: "flex flex-col gap-3 p-3",
                                style: "background: var(--glass-bg-subtle); border-radius: 12px; border: 1px solid var(--glass-border);",
                                
                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Upstream API Reference ID (Optional)" }
                                    ChipInput {
                                        values: form_openai_api,
                                        suggestions: openai_api_suggestions.read().clone(),
                                        max_items: Some(1),
                                        placeholder: "Select upstream provider...",
                                    }
                                }

                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Custom Upstream URL (Optional)" }
                                    input {
                                        class: "glass-input",
                                        placeholder: "https://api.openai.com/v1",
                                        value: "{form_openai_url.read()}",
                                        oninput: move |evt| form_openai_url.set(evt.value()),
                                    }
                                }

                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Target Upstream Model ID" }
                                    input {
                                        class: "glass-input font-mono",
                                        placeholder: "gpt-4o-mini",
                                        value: "{form_openai_model.read()}",
                                        oninput: move |evt| form_openai_model.set(evt.value()),
                                    }
                                }

                                div {
                                    class: "form-group",
                                    label { class: "form-label", "Secret Token (Optional)" }
                                    input {
                                        r#type: "password",
                                        class: "glass-input",
                                        value: "{form_openai_token.read()}",
                                        oninput: move |evt| form_openai_token.set(evt.value()),
                                    }
                                }
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Model Timeout in seconds (Optional)" }
                            input {
                                r#type: "number",
                                class: "glass-input",
                                placeholder: "e.g. 600 (defaults to system setting)",
                                value: "{form_timeout.read()}",
                                oninput: move |evt| form_timeout.set(evt.value()),
                            }
                            span { class: "form-hint", "Override idle/request timeout for this model in seconds." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Limit to Users (empty for all users)" }
                            ChipInput {
                                values: form_users,
                                suggestions: user_suggestions.read().clone(),
                                placeholder: "Add user restriction...",
                            }
                        }

                        div {
                            class: "flex items-center gap-2 mt-2",
                            input {
                                r#type: "checkbox",
                                id: "model-hidden-check",
                                checked: *form_hidden.read(),
                                onchange: move |evt| form_hidden.set(evt.checked()),
                            }
                            label { r#for: "model-hidden-check", class: "text-sm", "Hide from public /v1/models listing" }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| editing_model.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_model,
                                "Save Model"
                            }
                        }
                    }
                }
            }
        }
    }
}
