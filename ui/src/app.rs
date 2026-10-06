use crate::api::{fetch_auth_config, fetch_current_user, get_stored_token, logout, set_stored_token};
use crate::components::icons::*;
use crate::components::{GlassCard, GlassModal, Navbar, ToastContainer};
use crate::models::UserProfile;
use crate::state::{
    get_initial_tab, get_initial_theme, set_browser_path_for_tab, ActiveTab, Toast,
};
use crate::views::*;
use dioxus::prelude::*;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::JsCast;
use web_sys::window;

#[component]
pub fn App() -> Element {
    let mut active_tab = use_signal(get_initial_tab);
    let mut current_user = use_signal(|| Option::<UserProfile>::None);
    let is_dark = use_signal(get_initial_theme);
    let mut toasts = use_signal(Vec::<Toast>::new);
    let mut show_auth_modal = use_signal(|| false);
    let mut manual_token_input = use_signal(String::new);
    let mut oidc_enabled = use_signal(|| false);
    let mut is_auth_checking = use_signal(|| true);

    // Sync tab changes to browser URL path
    use_effect(move || {
        let tab = *active_tab.read();
        set_browser_path_for_tab(tab);
    });

    // Listen for browser back/forward buttons (popstate event)
    use_hook(move || {
        if let Some(w) = window() {
            let callback = Closure::wrap(Box::new(move |_: web_sys::Event| {
                active_tab.set(get_initial_tab());
            }) as Box<dyn FnMut(_)>);
            let _ = w.add_event_listener_with_callback(
                "popstate",
                callback.as_ref().unchecked_ref(),
            );
            callback.forget();
        }
    });

    // Fetch auth config & profile on mount
    let _ = use_resource(move || async move {
        if let Ok(cfg) = fetch_auth_config().await {
            oidc_enabled.set(cfg.oidc_enabled);
        }
        if let Ok(profile) = fetch_current_user().await {
            current_user.set(Some(profile));
        } else {
            current_user.set(None);
        }
        is_auth_checking.set(false);
    });

    let is_authenticated = current_user.read().is_some();

    rsx! {
        div {
            class: "app-shell",
            
            // Ambient Floating Background Glow Orbs
            div {
                class: "ambient-glow-container",
                div { class: "ambient-orb ambient-orb-1" }
                div { class: "ambient-orb ambient-orb-2" }
            }

            // Glass Navigation Bar
            Navbar {
                active_tab,
                current_user,
                is_dark,
                on_login_click: move |_| {
                    if let Some(tok) = get_stored_token() {
                        manual_token_input.set(tok);
                    }
                    show_auth_modal.set(true);
                },
                on_logout_click: move |_| {
                    spawn(async move {
                        let _ = logout().await;
                        current_user.set(None);
                        toasts.write().push(Toast {
                            id: js_sys::Date::now() as u64,
                            message: "Signed out successfully.".to_string(),
                            is_error: false,
                        });
                    });
                },
                on_api_key_click: move |_| {
                    if let Some(tok) = get_stored_token() {
                        manual_token_input.set(tok);
                    }
                    show_auth_modal.set(true);
                },
            }

            // Main Content Area
            main {
                class: "app-main",
                
                if *is_auth_checking.read() {
                    div {
                        class: "flex flex-col items-center justify-center",
                        style: "min-height: calc(100vh - 200px);",
                        GlassCard {
                            style: "padding: 2.5rem 3rem; text-align: center;",
                            glow: true,
                            div {
                                class: "brand-icon mb-4",
                                style: "width: 48px; height: 48px; margin: 0 auto 1rem;",
                                IconBolt { size: 24 }
                            }
                            h3 { class: "text-lg font-bold mb-2", "Connecting to DynamicRouter..." }
                            p { class: "text-xs text-muted", "Verifying authentication and security session" }
                        }
                    }
                } else if !is_authenticated {
                    // Dedicated Login Page when Unauthenticated
                    LoginView {
                        current_user,
                        toasts,
                        oidc_enabled,
                    }
                } else {
                    // Full App for Authenticated Users / Admins
                    match *active_tab.read() {
                        ActiveTab::Chat => rsx! { ChatView {} },
                        ActiveTab::Tokens => rsx! { TokensView { toasts } },
                        ActiveTab::UserLogs => rsx! { UserLogsView {} },
                        ActiveTab::UserMetrics => rsx! { UserMetricsView {} },
                        ActiveTab::AdminDashboard => rsx! { AdminDashboardView {} },
                        ActiveTab::AdminModels => rsx! { AdminModelsView { toasts } },
                        ActiveTab::AdminOpenAiApis => rsx! { AdminOpenAiApisView { toasts } },
                        ActiveTab::AdminBackends => rsx! { AdminBackendsView { toasts } },
                        ActiveTab::AdminUsers => rsx! { AdminUsersView { toasts } },
                        ActiveTab::AdminFactoryGpus => rsx! { AdminFactoryGpusView { toasts } },
                        ActiveTab::AdminInstances => rsx! { AdminInstancesView { toasts } },
                        ActiveTab::AdminAliases => rsx! { AdminAliasesView { toasts } },
                        ActiveTab::AdminSystemLogs => rsx! { AdminSystemLogsView { toasts } },
                    }
                }
            }

            // Auth / API Key Modal
            if *show_auth_modal.read() {
                GlassModal {
                    title: "Authentication & Access".to_string(),
                    on_close: move |_| show_auth_modal.set(false),
                    div {
                        class: "flex flex-col gap-4",
                        
                        div {
                            class: "flex flex-col gap-2 p-3",
                            style: "background: var(--glow-surface); border: 1px solid var(--glow-border); border-radius: 12px;",
                            h4 { class: "text-sm font-bold", "Single Sign-On (SSO / OIDC)" }
                            p { class: "text-xs text-muted", "Sign in using your organization's OpenID Connect identity provider." }
                            button {
                                class: "glass-btn btn-primary mt-2",
                                onclick: move |_| {
                                    if let Some(w) = window() {
                                        let _ = w.location().set_href("/auth/login");
                                    }
                                },
                                IconShield { size: 16 }
                                span { "Continue with OIDC SSO" }
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Manual Bearer Token / API Key" }
                            input {
                                r#type: "password",
                                class: "glass-input font-mono",
                                placeholder: "Enter Bearer API key or session token...",
                                value: "{manual_token_input.read()}",
                                oninput: move |evt| manual_token_input.set(evt.value()),
                            }
                            span { class: "form-hint", "Bearer token is stored locally in your browser to authorize requests." }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| show_auth_modal.set(false),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: move |_| {
                                    let tok = manual_token_input.read().trim().to_string();
                                    if tok.is_empty() {
                                        set_stored_token(None);
                                    } else {
                                        set_stored_token(Some(&tok));
                                    }
                                    spawn(async move {
                                        match fetch_current_user().await {
                                            Ok(profile) => {
                                                current_user.set(Some(profile));
                                                toasts.write().push(Toast {
                                                    id: js_sys::Date::now() as u64,
                                                    message: "Authenticated successfully.".to_string(),
                                                    is_error: false,
                                                });
                                                show_auth_modal.set(false);
                                            }
                                            Err(err) => {
                                                toasts.write().push(Toast {
                                                    id: js_sys::Date::now() as u64,
                                                    message: format!("Authentication failed: {err}"),
                                                    is_error: true,
                                                });
                                            }
                                        }
                                    });
                                },
                                "Authenticate"
                            }
                        }
                    }
                }
            }

            // Toast Notifications Container
            ToastContainer { toasts }
        }
    }
}
