use crate::api::{create_user_token, delete_user_token, fetch_user_tokens};
use crate::components::icons::*;
use crate::components::{GlassCard, GlassModal};
use crate::models::{CreatedTokenResponse, UserToken};
use crate::state::Toast;
use dioxus::prelude::*;
use web_sys::window;

fn load_tokens_action(
    mut is_loading: Signal<bool>,
    mut tokens: Signal<Vec<UserToken>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_user_tokens().await {
            Ok(list) => tokens.set(list),
            Err(err) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Error fetching tokens: {err}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn TokensView(toasts: Signal<Vec<Toast>>) -> Element {
    let tokens = use_signal(Vec::<UserToken>::new);
    let is_loading = use_signal(|| false);
    let mut created_token = use_signal(|| Option::<CreatedTokenResponse>::None);
    let mut copied = use_signal(|| false);

    let _ = use_resource(move || async move {
        load_tokens_action(is_loading, tokens, toasts);
    });

    let on_generate_token = move |_| {
        spawn(async move {
            match create_user_token().await {
                Ok(res) => {
                    created_token.set(Some(res));
                    load_tokens_action(is_loading, tokens, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to create token: {err}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let on_delete_token = move |hash: String| {
        spawn(async move {
            match delete_user_token(&hash).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Token revoked successfully.".to_string(),
                        is_error: false,
                    });
                    load_tokens_action(is_loading, tokens, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to revoke token: {err}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let copy_new_token = {
        move |_| {
            if let Some(res) = created_token.read().as_ref() {
                if let Some(w) = window() {
                    let nav = w.navigator();
                    let clip = nav.clipboard();
                    let _ = clip.write_text(&res.token);
                    copied.set(true);
                }
            }
        }
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
                            IconKey { size: 20 }
                            span { "Personal API Tokens" }
                        }
                        p { class: "card-description", "API tokens allow you to authenticate with the OpenAI-compatible REST endpoints (/v1/chat/completions, /v1/models)." }
                    }
                    button {
                        class: "glass-btn btn-primary",
                        onclick: on_generate_token,
                        IconPlus { size: 16 }
                        span { "Generate New Token" }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Token Hash" }
                                th { "Created At" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if tokens.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 3,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading tokens..." } else { "No API tokens found. Generate one to get started." }
                                    }
                                }
                            }
                            for token in tokens.read().iter() {
                                tr {
                                    key: "{token.token_hash}",
                                    td {
                                        span { class: "font-mono text-sm", "{token.token_hash}" }
                                    }
                                    td {
                                        span { class: "text-muted text-sm", "{token.created_at.as_deref().unwrap_or(\"—\")}" }
                                    }
                                    td {
                                        class: "text-right",
                                        button {
                                            class: "glass-btn btn-danger btn-sm",
                                            onclick: {
                                                let hash = token.token_hash.clone();
                                                move |_| on_delete_token(hash.clone())
                                            },
                                            IconTrash { size: 14 }
                                            span { "Revoke" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // New Token Created Modal
            if let Some(res) = created_token.read().clone() {
                GlassModal {
                    title: "Token Generated Successfully".to_string(),
                    on_close: move |_| {
                        created_token.set(None);
                        copied.set(false);
                    },
                    div {
                        class: "flex flex-col gap-3",
                        p { class: "text-sm text-muted", "{res.message}" }
                        div {
                            class: "flex items-center gap-2",
                            input {
                                class: "glass-input font-mono text-sm",
                                readonly: true,
                                value: "{res.token}",
                            }
                            button {
                                class: "glass-btn btn-primary btn-sm",
                                onclick: copy_new_token,
                                if *copied.read() {
                                    IconCheck { size: 14 }
                                    span { "Copied" }
                                } else {
                                    span { "Copy" }
                                }
                            }
                        }
                        div {
                            class: "flex items-start gap-2 p-3 rounded-lg",
                            style: "background: rgba(245, 158, 11, 0.1); border: 1px solid rgba(245, 158, 11, 0.25); color: #fbbf24; font-size: 0.8rem;",
                            IconAlertTriangle { size: 16 }
                            span { "Warning: This token will never be displayed again. Make sure to copy and store it in a secure location." }
                        }
                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| {
                                    created_token.set(None);
                                    copied.set(false);
                                },
                                "Done"
                            }
                        }
                    }
                }
            }
        }
    }
}
