use crate::api::{
    delete_admin_openai_api, fetch_admin_openai_apis, fetch_v1_models, save_admin_openai_api,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::OpenAiApiRecord;
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::HashSet;

pub fn suggest_corrected_url(url_str: &str) -> Option<(&'static str, &'static str)> {
    let trimmed = url_str.trim();
    if trimmed.is_empty() {
        return None;
    }
    let lower = trimmed.to_lowercase();
    if lower.contains("openrouter.ai") {
        if !lower.contains("/api/v1") {
            return Some((
                "https://openrouter.ai/api/v1",
                "OpenRouter API requires the '/api/v1' path prefix (e.g. https://openrouter.ai/api/v1).",
            ));
        }
    } else if lower.contains("api.groq.com") {
        if !lower.contains("/openai/v1") {
            return Some((
                "https://api.groq.com/openai/v1",
                "Groq OpenAI API requires the '/openai/v1' path prefix.",
            ));
        }
    } else if lower.contains("api.deepinfra.com") {
        if !lower.contains("/v1/openai") {
            return Some((
                "https://api.deepinfra.com/v1/openai",
                "DeepInfra OpenAI API requires the '/v1/openai' path prefix.",
            ));
        }
    } else if lower.contains("api.openai.com") {
        if !lower.contains("/v1") {
            return Some((
                "https://api.openai.com/v1",
                "OpenAI API uses the '/v1' path prefix.",
            ));
        }
    }
    None
}

fn load_apis_action(
    mut is_loading: Signal<bool>,
    mut apis: Signal<Vec<OpenAiApiRecord>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_openai_apis().await {
            Ok(list) => apis.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to fetch OpenAI APIs: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminOpenAiApisView(toasts: Signal<Vec<Toast>>) -> Element {
    let apis = use_signal(Vec::<OpenAiApiRecord>::new);
    let mut model_suggestions = use_signal(Vec::<String>::new);
    let is_loading = use_signal(|| false);
    let mut editing_api = use_signal(|| Option::<(OpenAiApiRecord, bool)>::None);

    // Form inputs
    let mut form_id = use_signal(String::new);
    let mut form_url = use_signal(String::new);
    let mut form_token = use_signal(String::new);
    let mut form_prefix = use_signal(String::new);
    let mut form_models = use_signal(Vec::<String>::new);
    let mut form_timeout = use_signal(String::new);

    let _ = use_resource(move || async move {
        load_apis_action(is_loading, apis, toasts);

        let mut defaults = vec![
            "gpt-4o".to_string(),
            "gpt-4o-mini".to_string(),
            "o1".to_string(),
            "o1-mini".to_string(),
            "o3-mini".to_string(),
            "claude-3-5-sonnet-latest".to_string(),
            "claude-3-5-haiku-latest".to_string(),
            "gemini-2.0-flash".to_string(),
            "gemini-2.0-pro-exp-02-05".to_string(),
            "deepseek-ai/DeepSeek-R1".to_string(),
            "deepseek-ai/DeepSeek-V3".to_string(),
        ];
        if let Ok(models) = fetch_v1_models().await {
            for m in models {
                if !defaults.contains(&m.id) {
                    defaults.push(m.id);
                }
            }
        }
        defaults.sort();
        model_suggestions.set(defaults);
    });

    let open_create_modal = move |_| {
        form_id.set(String::new());
        form_url.set("https://api.openai.com/v1".to_string());
        form_token.set(String::new());
        form_prefix.set(String::new());
        form_models.set(Vec::new());
        form_timeout.set(String::new());

        editing_api.set(Some((
            OpenAiApiRecord {
                id: String::new(),
                url: "https://api.openai.com/v1".to_string(),
                token: None,
                prefix: None,
                models: None,
                timeout: None,
                headers: None,
            },
            true,
        )));
    };

    let save_api = move |_| {
        let (_, is_new) = match editing_api.read().clone() {
            Some(m) => m,
            None => return,
        };

        let id = form_id.read().trim().to_string();
        let url = form_url.read().trim().to_string();
        if id.is_empty() || url.is_empty() {
            return;
        }

        let token = if form_token.read().trim().is_empty() {
            None
        } else {
            Some(form_token.read().trim().to_string())
        };

        let prefix = if form_prefix.read().trim().is_empty() {
            None
        } else {
            Some(form_prefix.read().trim().to_string())
        };

        let timeout = form_timeout.read().trim().parse::<u32>().ok();

        let models = if form_models.read().is_empty() {
            None
        } else {
            let p_clean = prefix.as_deref().map(|p| p.trim_end_matches('/'));
            let cleaned: HashSet<String> = form_models
                .read()
                .iter()
                .map(|m| {
                    let mut s = m.trim().to_string();
                    if let Some(p) = p_clean {
                        while let Some(stripped) = s.strip_prefix(&format!("{p}/")).or_else(|| s.strip_prefix(p)) {
                            let next = stripped.trim_start_matches('/').to_string();
                            if next == s || next.is_empty() {
                                break;
                            }
                            s = next;
                        }
                    }
                    s
                })
                .filter(|s| !s.is_empty())
                .collect();
            if cleaned.is_empty() { None } else { Some(cleaned) }
        };

        let payload = OpenAiApiRecord {
            id,
            url,
            token,
            prefix,
            models,
            timeout,
            headers: None,
        };

        spawn(async move {
            match save_admin_openai_api(&payload, is_new).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "OpenAI upstream saved.".to_string(),
                        is_error: false,
                    });
                    editing_api.set(None);
                    load_apis_action(is_loading, apis, toasts);
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

    let on_delete_api = move |id: String| {
        spawn(async move {
            match delete_admin_openai_api(&id).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "OpenAI upstream deleted.".to_string(),
                        is_error: false,
                    });
                    load_apis_action(is_loading, apis, toasts);
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
                            IconGlobe { size: 20 }
                            span { "OpenAI-Compatible Upstream Providers" }
                        }
                        p { class: "card-description", "Connect external inference providers (OpenAI, Groq, Together, DeepInfra, OpenRouter) to route models transparently." }
                    }
                    button {
                        class: "glass-btn btn-primary",
                        onclick: open_create_modal,
                        IconPlus { size: 16 }
                        span { "Add Upstream Provider" }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Provider ID" }
                                th { "Endpoint URL" }
                                th { "Prefix" }
                                th { "Models" }
                                th { "Timeout" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if apis.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 6,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading upstreams..." } else { "No upstream providers configured." }
                                    }
                                }
                            }
                            for api in apis.read().iter() {
                                tr {
                                    key: "{api.id}",
                                    td { span { class: "font-mono font-bold text-sm", "{api.id}" } }
                                    td { span { class: "font-mono text-xs text-muted", "{api.url}" } }
                                    td {
                                        if let Some(ref p) = api.prefix {
                                            GlassBadge { "{p}" }
                                        } else {
                                            span { class: "text-muted text-xs", "—" }
                                        }
                                    }
                                    td {
                                        if let Some(ref m) = api.models {
                                            GlassBadge { variant: "glow", "{m.len()} models" }
                                        } else {
                                            GlassBadge { "Dynamic / All" }
                                        }
                                    }
                                    td {
                                        span {
                                            class: "text-sm font-mono text-muted",
                                            match api.timeout {
                                                Some(t) => format!("{t}s"),
                                                None => "Default".to_string(),
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
                                                    let a = api.clone();
                                                    move |_| {
                                                        form_id.set(a.id.clone());
                                                        form_url.set(a.url.clone());
                                                        form_token.set(a.token.clone().unwrap_or_default());
                                                        form_prefix.set(a.prefix.clone().unwrap_or_default());
                                                        let mut m_list: Vec<String> = a.models
                                                            .as_ref()
                                                            .map(|m| m.iter().cloned().collect())
                                                            .unwrap_or_default();
                                                        m_list.sort();
                                                        form_models.set(m_list);
                                                        form_timeout.set(a.timeout.map(|t| t.to_string()).unwrap_or_default());

                                                        editing_api.set(Some((a.clone(), false)));
                                                    }
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "glass-btn btn-danger btn-sm",
                                                onclick: {
                                                    let id = api.id.clone();
                                                    move |_| on_delete_api(id.clone())
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

            // Modal
            if let Some((_, is_new)) = editing_api.read().clone() {
                GlassModal {
                    title: if is_new { "Add Upstream Provider".to_string() } else { format!("Edit Provider: {}", form_id.read()) },
                    on_close: move |_| editing_api.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        
                        if is_new {
                            div {
                                class: "form-group",
                                label { class: "form-label text-xs text-muted", "Quick Presets" }
                                div {
                                    class: "flex flex-wrap gap-2",
                                    button {
                                        r#type: "button",
                                        class: "glass-btn btn-secondary btn-sm",
                                        onclick: move |_| {
                                            form_id.set("openrouter".to_string());
                                            form_url.set("https://openrouter.ai/api/v1".to_string());
                                            form_prefix.set("or/".to_string());
                                        },
                                        "OpenRouter"
                                    }
                                    button {
                                        r#type: "button",
                                        class: "glass-btn btn-secondary btn-sm",
                                        onclick: move |_| {
                                            form_id.set("openai".to_string());
                                            form_url.set("https://api.openai.com/v1".to_string());
                                            form_prefix.set("openai/".to_string());
                                        },
                                        "OpenAI"
                                    }
                                    button {
                                        r#type: "button",
                                        class: "glass-btn btn-secondary btn-sm",
                                        onclick: move |_| {
                                            form_id.set("groq".to_string());
                                            form_url.set("https://api.groq.com/openai/v1".to_string());
                                            form_prefix.set("groq/".to_string());
                                        },
                                        "Groq"
                                    }
                                    button {
                                        r#type: "button",
                                        class: "glass-btn btn-secondary btn-sm",
                                        onclick: move |_| {
                                            form_id.set("deepinfra".to_string());
                                            form_url.set("https://api.deepinfra.com/v1/openai".to_string());
                                            form_prefix.set("deepinfra/".to_string());
                                        },
                                        "DeepInfra"
                                    }
                                }
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Provider ID (e.g. openrouter, groq)" }
                            input {
                                class: "glass-input font-mono",
                                disabled: !is_new,
                                value: "{form_id.read()}",
                                oninput: move |evt| form_id.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Endpoint Base URL" }
                            input {
                                class: "glass-input font-mono",
                                placeholder: "https://api.openai.com/v1 (or https://openrouter.ai/api/v1)",
                                value: "{form_url.read()}",
                                oninput: move |evt| form_url.set(evt.value()),
                            }
                            if let Some((suggested, reason)) = suggest_corrected_url(&form_url.read()) {
                                div {
                                    style: "margin-top: 0.35rem; padding: 0.5rem 0.75rem; border-radius: 8px; background: rgba(234, 179, 8, 0.1); border: 1px solid rgba(234, 179, 8, 0.25); display: flex; flex-direction: column; gap: 0.25rem;",
                                    div {
                                        class: "flex items-center justify-between gap-2",
                                        span {
                                            class: "text-xs font-medium text-warning",
                                            "💡 Did you mean: "
                                            strong { class: "font-mono", "{suggested}" }
                                        }
                                        button {
                                            r#type: "button",
                                            class: "glass-btn btn-secondary btn-sm",
                                            style: "padding: 2px 8px; font-size: 11px; height: 22px;",
                                            onclick: move |_| form_url.set(suggested.to_string()),
                                            "Use Suggestion"
                                        }
                                    }
                                    span { class: "text-xs text-muted", "{reason} (You may still override and keep your custom URL)" }
                                }
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "API Secret Token (Optional)" }
                            input {
                                r#type: "password",
                                class: "glass-input",
                                value: "{form_token.read()}",
                                oninput: move |evt| form_token.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Model Prefix (Optional, e.g. 'groq/' or 'openai/')" }
                            input {
                                class: "glass-input",
                                value: "{form_prefix.read()}",
                                oninput: move |evt| form_prefix.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Request Timeout in seconds (Optional)" }
                            input {
                                r#type: "number",
                                class: "glass-input",
                                placeholder: "e.g. 600 (defaults to system setting)",
                                value: "{form_timeout.read()}",
                                oninput: move |evt| form_timeout.set(evt.value()),
                            }
                            span { class: "form-hint", "HTTP client request timeout in seconds for this upstream provider." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Handled Models (e.g. gpt-4o, claude-3.5-sonnet)" }
                            ChipInput {
                                values: form_models,
                                suggestions: model_suggestions.read().clone(),
                                placeholder: "e.g. gpt-4o, gpt-4o-mini...",
                            }
                            span { class: "form-hint", "Press Enter or comma to add each model ID." }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| editing_api.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_api,
                                "Save Provider"
                            }
                        }
                    }
                }
            }
        }
    }
}
