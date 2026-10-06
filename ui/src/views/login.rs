use crate::api::{fetch_current_user, set_stored_token};
use crate::components::icons::*;
use crate::components::GlassCard;
use crate::models::UserProfile;
use crate::state::Toast;
use dioxus::prelude::*;
use web_sys::window;

fn perform_manual_login(
    mut is_authenticating: Signal<bool>,
    manual_token_input: Signal<String>,
    mut current_user: Signal<Option<UserProfile>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    let tok = manual_token_input.read().trim().to_string();
    if tok.is_empty() {
        return;
    }

    set_stored_token(Some(&tok));
    is_authenticating.set(true);

    spawn(async move {
        match fetch_current_user().await {
            Ok(profile) => {
                current_user.set(Some(profile));
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: "Authenticated successfully.".to_string(),
                    is_error: false,
                });
            }
            Err(err) => {
                set_stored_token(None);
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Authentication failed: {err}"),
                    is_error: true,
                });
            }
        }
        is_authenticating.set(false);
    });
}

#[component]
pub fn LoginView(
    current_user: Signal<Option<UserProfile>>,
    toasts: Signal<Vec<Toast>>,
    #[allow(unused_variables)] oidc_enabled: Signal<bool>,
) -> Element {
    let mut manual_token_input = use_signal(String::new);
    let mut show_token_field = use_signal(|| false);
    let is_authenticating = use_signal(|| false);

    let on_oidc_login = move |_| {
        if let Some(w) = window() {
            let _ = w.location().set_href("/auth/login");
        }
    };

    rsx! {
        div {
            class: "flex flex-col items-center justify-center",
            style: "min-height: calc(100vh - 160px); padding: 1.5rem;",
            
            GlassCard {
                style: "max-width: 480px; width: 100%; text-align: center; padding: 2.75rem 2.25rem;",
                glow: true,

                // Brand Icon
                div {
                    class: "flex items-center justify-center mb-4",
                    div {
                        class: "brand-icon",
                        style: "width: 56px; height: 56px; border-radius: 16px;",
                        IconBolt { size: 28 }
                    }
                }

                // Headings
                h1 {
                    style: "font-family: var(--font-display); font-size: 1.75rem; font-weight: 800; margin-bottom: 0.5rem; letter-spacing: -0.02em;",
                    "Welcome to DynamicRouter"
                }
                p {
                    class: "text-sm text-muted mb-4",
                    "High-performance dynamic model inference, multi-backend routing, and cluster orchestration."
                }

                div {
                    class: "flex flex-col gap-3 mt-4",
                    
                    // Main OIDC Login Button
                    button {
                        class: "glass-btn btn-primary",
                        style: "padding: 0.85rem 1.5rem; font-size: 1rem; width: 100%; justify-content: center; gap: 0.65rem;",
                        onclick: on_oidc_login,
                        IconShield { size: 18 }
                        span { "Sign In with SSO / OIDC" }
                    }

                    // Divider
                    div {
                        class: "flex items-center gap-3 my-2",
                        div { style: "flex: 1; height: 1px; background: var(--glass-border);" }
                        span { class: "text-xs text-muted uppercase", "or" }
                        div { style: "flex: 1; height: 1px; background: var(--glass-border);" }
                    }

                    // Secondary: Manual Token Login
                    if !*show_token_field.read() {
                        button {
                            class: "glass-btn btn-secondary btn-sm",
                            style: "width: 100%; justify-content: center; gap: 0.5rem;",
                            onclick: move |_| show_token_field.set(true),
                            IconKey { size: 15 }
                            span { "Sign In with API Key / Token" }
                        }
                    } else {
                        div {
                            class: "flex flex-col gap-2 text-left",
                            label { class: "form-label text-xs", "Bearer API Token" }
                            input {
                                r#type: "password",
                                class: "glass-input font-mono text-sm",
                                placeholder: "sk-dr-... or session token",
                                value: "{manual_token_input.read()}",
                                oninput: move |evt| manual_token_input.set(evt.value()),
                                onkeydown: move |evt: KeyboardEvent| {
                                    if evt.key() == Key::Enter {
                                        perform_manual_login(is_authenticating, manual_token_input, current_user, toasts);
                                    }
                                }
                            }
                            button {
                                class: "glass-btn btn-secondary",
                                style: "width: 100%; justify-content: center; margin-top: 0.25rem;",
                                disabled: *is_authenticating.read() || manual_token_input.read().trim().is_empty(),
                                onclick: move |_| perform_manual_login(is_authenticating, manual_token_input, current_user, toasts),
                                if *is_authenticating.read() { "Authenticating..." } else { "Authenticate" }
                            }
                        }
                    }
                }
            }
        }
    }
}
