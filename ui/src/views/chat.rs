use crate::api::{fetch_v1_models, send_chat_completion};
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard};
use crate::models::{ChatCompletionRequest, ChatMessage, ModelListItem};
use dioxus::prelude::*;

fn send_chat_action(
    mut is_loading: Signal<bool>,
    mut user_input: Signal<String>,
    selected_model: Signal<String>,
    system_prompt: Signal<String>,
    mut messages: Signal<Vec<ChatMessage>>,
    mut error_msg: Signal<Option<String>>,
    temperature: Signal<f32>,
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
    messages.set(current_msgs.clone());

    let mut api_messages = Vec::new();
    let sys = system_prompt.read().trim().to_string();
    if !sys.is_empty() {
        api_messages.push(ChatMessage {
            role: "system".to_string(),
            content: sys,
        });
    }
    api_messages.extend(current_msgs);

    let req = ChatCompletionRequest {
        model: model_id,
        messages: api_messages,
        temperature: Some(*temperature.read()),
        max_tokens: None,
    };

    is_loading.set(true);
    spawn(async move {
        match send_chat_completion(&req).await {
            Ok(resp) => {
                if let Some(first_choice) = resp.choices.first() {
                    let mut msgs = messages.read().clone();
                    msgs.push(first_choice.message.clone());
                    messages.set(msgs);
                }
            }
            Err(err) => {
                error_msg.set(Some(err));
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn ChatView() -> Element {
    let mut models = use_signal(Vec::<ModelListItem>::new);
    let mut selected_model = use_signal(String::new);
    let mut system_prompt = use_signal(|| "You are a helpful, brilliant AI assistant powered by DynamicRouter.".to_string());
    let mut user_input = use_signal(String::new);
    let mut messages = use_signal(Vec::<ChatMessage>::new);
    let is_loading = use_signal(|| false);
    let mut error_msg = use_signal(|| Option::<String>::None);
    let mut temperature = use_signal(|| 0.7f32);
    let mut show_system_drawer = use_signal(|| false);

    // Fetch models on mount
    let _ = use_resource(move || async move {
        match fetch_v1_models().await {
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

                        div {
                            class: "form-group",
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
                                div {
                                    style: "white-space: pre-wrap;",
                                    "{msg.content}"
                                }
                            }
                        }
                        if *is_loading.read() {
                            div {
                                class: "chat-bubble bubble-assistant",
                                div { class: "bubble-role-badge", "AI Assistant" }
                                div {
                                    class: "flex items-center gap-2",
                                    span { class: "status-dot status-dot-pulse" }
                                    span { class: "text-sm text-muted", "Generating response..." }
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
                                    send_chat_action(is_loading, user_input, selected_model, system_prompt, messages, error_msg, temperature);
                                }
                            }
                        }
                        button {
                            class: "glass-btn btn-primary send-btn",
                            disabled: *is_loading.read() || user_input.read().trim().is_empty(),
                            onclick: move |_| send_chat_action(is_loading, user_input, selected_model, system_prompt, messages, error_msg, temperature),
                            span { "Send" }
                            IconSend { size: 15 }
                        }
                    }
                }
            }
        }
    }
}
