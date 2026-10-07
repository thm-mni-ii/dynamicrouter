use crate::api::{fetch_my_models, send_chat_completion, send_chat_completion_stream};
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard};
use crate::models::{ChatCompletionRequest, ChatMessage, UserModelStatus};
use dioxus::prelude::*;

fn scroll_chat_to_bottom() {
    if let Some(w) = web_sys::window() {
        if let Some(doc) = w.document() {
            if let Some(el) = doc.get_element_by_id("chat-messages-container") {
                el.set_scroll_top(el.scroll_height());
            }
        }
    }
}

fn send_chat_action(
    mut is_loading: Signal<bool>,
    mut user_input: Signal<String>,
    selected_model: Signal<String>,
    system_prompt: Signal<String>,
    mut messages: Signal<Vec<ChatMessage>>,
    mut error_msg: Signal<Option<String>>,
    temperature: Signal<f32>,
    stream_enabled: Signal<bool>,
) {
    let text = user_input.read().trim().to_string();
    let model_id = selected_model.read().clone();
    if text.is_empty() || model_id.is_empty() || *is_loading.read() {
        return;
    }

    user_input.set(String::new());
    error_msg.set(None);

    let user_msg = ChatMessage {
        role: "user".to_string(),
        content: text,
    };

    let mut current_msgs = messages.read().clone();
    current_msgs.push(user_msg);
    
    // Add placeholder assistant message
    current_msgs.push(ChatMessage {
        role: "assistant".to_string(),
        content: String::new(),
    });
    messages.set(current_msgs.clone());
    scroll_chat_to_bottom();

    let mut api_messages = Vec::new();
    let sys = system_prompt.read().trim().to_string();
    if !sys.is_empty() {
        api_messages.push(ChatMessage {
            role: "system".to_string(),
            content: sys,
        });
    }
    // Exclude the placeholder assistant message from the API request
    if current_msgs.len() > 1 {
        api_messages.extend(current_msgs[..current_msgs.len() - 1].to_vec());
    } else {
        api_messages.extend(current_msgs);
    }

    let is_streaming = *stream_enabled.read();
    let req = ChatCompletionRequest {
        model: model_id,
        messages: api_messages,
        temperature: Some(*temperature.read()),
        max_tokens: None,
        stream: Some(is_streaming),
    };

    is_loading.set(true);
    spawn(async move {
        if is_streaming {
            let res = send_chat_completion_stream(&req, move |delta| {
                let mut msgs = messages.read().clone();
                if let Some(last) = msgs.last_mut() {
                    if last.role == "assistant" {
                        last.content.push_str(delta);
                        messages.set(msgs);
                        scroll_chat_to_bottom();
                    }
                }
            })
            .await;

            if let Err(err) = res {
                let mut msgs = messages.read().clone();
                if let Some(last) = msgs.last() {
                    if last.role == "assistant" && last.content.is_empty() {
                        msgs.pop();
                        messages.set(msgs);
                    }
                }
                error_msg.set(Some(err));
            }
        } else {
            match send_chat_completion(&req).await {
                Ok(resp) => {
                    if let Some(first_choice) = resp.choices.first() {
                        let mut msgs = messages.read().clone();
                        if let Some(last) = msgs.last_mut() {
                            if last.role == "assistant" {
                                last.content = first_choice.message.content.clone();
                                messages.set(msgs);
                                scroll_chat_to_bottom();
                            }
                        }
                    }
                }
                Err(err) => {
                    let mut msgs = messages.read().clone();
                    if let Some(last) = msgs.last() {
                        if last.role == "assistant" && last.content.is_empty() {
                            msgs.pop();
                            messages.set(msgs);
                        }
                    }
                    error_msg.set(Some(err));
                }
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn ChatView() -> Element {
    let mut models = use_signal(Vec::<UserModelStatus>::new);
    let mut selected_model = use_signal(String::new);
    let mut system_prompt = use_signal(|| "You are a helpful, brilliant AI assistant powered by DynamicRouter.".to_string());
    let mut user_input = use_signal(String::new);
    let mut messages = use_signal(Vec::<ChatMessage>::new);
    let is_loading = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut temperature = use_signal(|| 0.7f32);
    let mut stream_enabled = use_signal(|| true);
    let mut show_system_drawer = use_signal(|| false);

    // Fetch rich models on mount
    let _ = use_resource(move || async move {
        match fetch_my_models().await {
            Ok(list) => {
                if !list.is_empty() && selected_model.read().is_empty() {
                    selected_model.set(list[0].id.clone());
                }
                models.set(list);
            }
            Err(e) => {
                error_msg.set(Some(format!("Failed to load models: {e}")));
            }
        }
    });

    let current_selected_id = selected_model.read().clone();
    let active_model_info = models
        .read()
        .iter()
        .find(|m| m.id == current_selected_id)
        .cloned();

    rsx! {
        div {
            class: "chat-layout",
            
            // Sidebar Controls
            div {
                class: "chat-sidebar",
                
                GlassCard {
                    class: "chat-sidebar-card",
                    
                    div {
                        class: "chat-sidebar-content",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Target Model" }
                            select {
                                class: "glass-select",
                                value: "{selected_model.read()}",
                                onchange: move |evt| selected_model.set(evt.value()),
                                if models.read().is_empty() {
                                    option { value: "", "Loading models..." }
                                }
                                for m in models.read().iter() {
                                    option {
                                        key: "{m.id}",
                                        value: "{m.id}",
                                        "{m.name}"
                                    }
                                }
                            }
                        }

                        // Selected Model Live Status Badge & Telemetry
                        if let Some(info) = active_model_info {
                            div {
                                class: "model-telemetry-panel",
                                
                                div {
                                    class: "model-telemetry-header",
                                    span { class: "model-telemetry-title", "Status" }
                                    if info.status == "running" {
                                        GlassBadge {
                                            variant: "glow",
                                            "🟢 Ready ({info.active_instances} active)"
                                        }
                                    } else if info.status == "upstream" {
                                        GlassBadge {
                                            variant: "primary",
                                            "🟣 Cloud Upstream"
                                        }
                                    } else {
                                        GlassBadge {
                                            variant: "secondary",
                                            "⚪ Cold Start Needed"
                                        }
                                    }
                                }

                                div {
                                    class: "model-telemetry-grid",
                                    div {
                                        class: "model-telemetry-item",
                                        span { class: "model-telemetry-label", "Queue Depth" }
                                        span {
                                            class: "model-telemetry-value",
                                            "{info.total_queue_length}"
                                            if info.user_queue_length > 0 {
                                                " ({info.user_queue_length} you)"
                                            }
                                        }
                                    }
                                    div {
                                        class: "model-telemetry-item",
                                        span { class: "model-telemetry-label", "Throughput" }
                                        span { class: "model-telemetry-value text-primary", "{info.total_output_tps:.1} tp/s" }
                                    }
                                }

                                if info.status == "idle" {
                                    div {
                                        class: "model-telemetry-notice",
                                        IconClock { size: 14 }
                                        span { "First prompt boots container (this will take a while)" }
                                    }
                                }
                            }
                        }

                        div {
                            class: "form-group mt-2",
                            div {
                                class: "flex items-center justify-between",
                                label { class: "form-label", "System Prompt" }
                                button {
                                    class: "glass-btn btn-secondary btn-sm",
                                    onclick: move |_| {
                                        let cur = *show_system_drawer.read();
                                        show_system_drawer.set(!cur);
                                    },
                                    if *show_system_drawer.read() { "Hide" } else { "Configure" }
                                }
                            }
                            if *show_system_drawer.read() {
                                textarea {
                                    class: "glass-textarea mt-2",
                                    value: "{system_prompt.read()}",
                                    oninput: move |evt| system_prompt.set(evt.value()),
                                    rows: 4,
                                    placeholder: "Enter custom instructions..."
                                }
                            }
                        }

                        div {
                            class: "form-group",
                            div {
                                class: "flex items-center justify-between",
                                label { class: "form-label", "Temperature: {temperature.read():.2}" }
                            }
                            input {
                                r#type: "range",
                                min: "0.0",
                                max: "2.0",
                                step: "0.05",
                                class: "w-full",
                                value: "{temperature.read()}",
                                oninput: move |evt| {
                                    if let Ok(val) = evt.value().parse::<f32>() {
                                        temperature.set(val);
                                    }
                                }
                            }
                        }

                        div {
                            class: "form-group",
                            label {
                                class: "flex items-center justify-between cursor-pointer",
                                span { class: "form-label mb-0", "Stream Responses" }
                                input {
                                    r#type: "checkbox",
                                    class: "glass-checkbox",
                                    checked: *stream_enabled.read(),
                                    onchange: move |evt| stream_enabled.set(evt.checked()),
                                }
                            }
                        }
                    }

                    div {
                        class: "chat-sidebar-footer",
                        button {
                            class: "glass-btn btn-secondary btn-sm",
                            onclick: move |_| {
                                messages.set(Vec::new());
                                error_msg.set(None);
                            },
                            IconTrash { size: 14 }
                            span { "Clear History" }
                        }
                        GlassBadge {
                            variant: "glow",
                            pulse: *is_loading.read(),
                            if *is_loading.read() { "Generating..." } else { "Ready" }
                        }
                    }
                }
            }

            // Chat Viewport
            div {
                class: "chat-viewport",
                
                GlassCard {
                    class: "chat-main-card",
                    style: "height: 100%; display: flex; flex-direction: column; padding: 0; min-height: 0;",
                    
                    // Messages Thread
                    div {
                        id: "chat-messages-container",
                        class: "chat-messages",
                        if messages.read().is_empty() {
                            div {
                                class: "chat-empty-state",
                                div {
                                    class: "chat-empty-icon-wrapper",
                                    IconChat { size: 32 }
                                }
                                h4 { class: "chat-empty-title", "DynamicRouter Playground" }
                                p { class: "chat-empty-desc", "Select a model, adjust parameters, and send a message to start prompting." }
                            }
                        }
                        for (idx, msg) in messages.read().iter().enumerate() {
                            div {
                                key: "{idx}",
                                class: if msg.role == "user" { "chat-bubble bubble-user" } else { "chat-bubble bubble-assistant" },
                                div {
                                    class: "bubble-role-badge",
                                    if msg.role == "user" { "You" } else { "AI Assistant" }
                                }
                                if idx + 1 == messages.read().len() && *is_loading.read() && msg.role == "assistant" && msg.content.is_empty() {
                                    div {
                                        class: "flex items-center gap-2",
                                        span { class: "status-dot status-dot-pulse" }
                                        span { class: "text-sm text-muted", "Generating response..." }
                                    }
                                } else {
                                    div {
                                        style: "white-space: pre-wrap;",
                                        "{msg.content}"
                                        if idx + 1 == messages.read().len() && *is_loading.read() && msg.role == "assistant" {
                                            span { class: "streaming-cursor" }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // Error notice if any
                    if let Some(err) = error_msg.read().as_ref() {
                        div {
                            class: "chat-error-banner",
                            IconAlertTriangle { size: 16 }
                            span { "{err}" }
                        }
                    }

                    // Prompt Input Area
                    div {
                        class: "chat-input-container",
                        textarea {
                            class: "glass-input chat-input-textarea",
                            placeholder: "Type your message... (Shift+Enter for newline, Enter to send)",
                            value: "{user_input.read()}",
                            disabled: *is_loading.read(),
                            oninput: move |evt| user_input.set(evt.value()),
                            onkeydown: move |evt: KeyboardEvent| {
                                if evt.key() == Key::Enter && !evt.modifiers().shift() {
                                    evt.prevent_default();
                                    send_chat_action(is_loading, user_input, selected_model, system_prompt, messages, error_msg, temperature, stream_enabled);
                                }
                            }
                        }
                        button {
                            class: "glass-btn btn-primary send-btn",
                            disabled: *is_loading.read() || user_input.read().trim().is_empty(),
                            onclick: move |_| send_chat_action(is_loading, user_input, selected_model, system_prompt, messages, error_msg, temperature, stream_enabled),
                            span { "Send" }
                            IconSend { size: 15 }
                        }
                    }
                }
            }
        }
    }
}
