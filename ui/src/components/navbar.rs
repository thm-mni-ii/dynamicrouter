use crate::components::icons::*;
use crate::models::UserProfile;
use crate::state::{
    apply_lighting_to_dom, apply_theme_to_dom, hex_to_rgb_str, ActiveTab,
    LIGHTING_PRESETS,
};
use dioxus::prelude::*;

#[component]
pub fn Navbar(
    active_tab: Signal<ActiveTab>,
    current_user: Signal<Option<UserProfile>>,
    is_dark: Signal<bool>,
    on_login_click: EventHandler<()>,
    on_logout_click: EventHandler<()>,
    #[allow(unused_variables)] on_api_key_click: EventHandler<()>,
) -> Element {
    let mut show_palette = use_signal(|| false);
    let mut show_admin_menu = use_signal(|| false);
    let mut custom_hex = use_signal(|| "#00f2fe".to_string());

    let user_opt = current_user.read().clone();
    let is_logged_in = user_opt.is_some();
    let is_admin = user_opt.as_ref().map(|u| u.is_admin).unwrap_or(false);

    let toggle_theme = move |_| {
        let new_theme = !*is_dark.read();
        is_dark.set(new_theme);
        apply_theme_to_dom(new_theme);
    };

    rsx! {
        nav {
            class: "glass-nav",
            
            // Brand Logo & Status
            div {
                class: "nav-brand",
                onclick: move |_| {
                    if is_logged_in {
                        active_tab.set(ActiveTab::Chat);
                        show_admin_menu.set(false);
                    }
                },
                div {
                    class: "brand-icon",
                    IconBolt { size: 18 }
                }
                span { "DynamicRouter" }
            }

            // Navigation Tabs (Only rendered when logged in)
            if is_logged_in {
                div {
                    class: "nav-tabs",
                    
                    // User Navigation Tabs
                    button {
                        class: if *active_tab.read() == ActiveTab::Chat { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(ActiveTab::Chat);
                            show_admin_menu.set(false);
                        },
                        IconChat { size: 16 }
                        span { "Chat" }
                    }
                    button {
                        class: if *active_tab.read() == ActiveTab::UserModels { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(ActiveTab::UserModels);
                            show_admin_menu.set(false);
                        },
                        IconBot { size: 16 }
                        span { "Models" }
                    }
                    button {
                        class: if *active_tab.read() == ActiveTab::Tokens { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(ActiveTab::Tokens);
                            show_admin_menu.set(false);
                        },
                        IconKey { size: 16 }
                        span { "Tokens" }
                    }
                    button {
                        class: if *active_tab.read() == ActiveTab::UserLogs { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(ActiveTab::UserLogs);
                            show_admin_menu.set(false);
                        },
                        IconLogs { size: 16 }
                        span { "Logs" }
                    }
                    button {
                        class: if *active_tab.read() == ActiveTab::UserMetrics { "nav-tab active" } else { "nav-tab" },
                        onclick: move |_| {
                            active_tab.set(ActiveTab::UserMetrics);
                            show_admin_menu.set(false);
                        },
                        IconMetrics { size: 16 }
                        span { "Metrics" }
                    }

                    // Admin Dropdown at the end of the navbar
                    if is_admin {
                        div { class: "nav-tab-divider" }
                        div {
                            class: "admin-dropdown-wrapper",
                            button {
                                class: if active_tab.read().is_admin() { "nav-tab active admin-nav-btn" } else { "nav-tab admin-nav-btn" },
                                onclick: move |_| {
                                    let cur = *show_admin_menu.read();
                                    show_admin_menu.set(!cur);
                                    show_palette.set(false);
                                },
                                IconShield { size: 15 }
                                span { "Admin" }
                                IconChevronDown {
                                    size: 13,
                                    class: if *show_admin_menu.read() { "dropdown-arrow rotate-180" } else { "dropdown-arrow" }
                                }
                            }

                            if *show_admin_menu.read() {
                                div {
                                    class: "dropdown-backdrop",
                                    onclick: move |_| show_admin_menu.set(false),
                                }
                                div {
                                    class: "admin-dropdown-menu",
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminDashboard { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminDashboard);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconDashboard { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Dashboard" }
                                            span { class: "admin-item-desc", "Cluster health & real-time metrics" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminModels { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminModels);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconBot { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Models" }
                                            span { class: "admin-item-desc", "Model routing & scaling policies" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminOpenAiApis { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminOpenAiApis);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconGlobe { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "OpenAI APIs" }
                                            span { class: "admin-item-desc", "Upstream inference providers" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminBackends { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminBackends);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconServer { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Backends" }
                                            span { class: "admin-item-desc", "Inference runners & engine args" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminUsers { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminUsers);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconUsers { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Users & Roles" }
                                            span { class: "admin-item-desc", "Access management & privileges" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminRateLimits { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminRateLimits);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconGauge { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Rate Limits" }
                                            span { class: "admin-item-desc", "Multi-tier token & request quotas" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminFactoryGpus { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminFactoryGpus);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconGpu { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "GPUs & Hardware" }
                                            span { class: "admin-item-desc", "Cloud & cluster spawner specs" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminInstances { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminInstances);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconMonitor { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Instances" }
                                            span { class: "admin-item-desc", "Active worker containers" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminAliases { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminAliases);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconShuffle { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Aliases" }
                                            span { class: "admin-item-desc", "Model routing aliases & fallbacks" }
                                        }
                                    }
                                    button {
                                        class: if *active_tab.read() == ActiveTab::AdminSystemLogs { "admin-dropdown-item active" } else { "admin-dropdown-item" },
                                        onclick: move |_| {
                                            active_tab.set(ActiveTab::AdminSystemLogs);
                                            show_admin_menu.set(false);
                                        },
                                        div { class: "admin-item-icon", IconAudit { size: 16 } }
                                        div {
                                            class: "admin-item-text",
                                            span { class: "admin-item-title", "Audit Logs" }
                                            span { class: "admin-item-desc", "Global cluster request trail" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else {
                div { style: "flex: 1;" }
            }

            // Right Actions: Lighting Picker, Theme Toggle, Auth
            div {
                class: "nav-actions",

                // Configurable Lighting Picker
                div {
                    class: "lighting-picker-wrapper",
                    button {
                        class: "color-swatch-btn",
                        title: "Configure Ambient Lighting Glow",
                        onclick: move |_| {
                            let current = *show_palette.read();
                            show_palette.set(!current);
                            show_admin_menu.set(false);
                        },
                    }

                    if *show_palette.read() {
                        div {
                            class: "color-palette-popover",
                            div {
                                class: "flex items-center justify-between",
                                span { class: "text-xs font-bold uppercase text-muted", "Ambient Lighting" }
                                button {
                                    class: "icon-btn",
                                    style: "width: 22px; height: 22px;",
                                    onclick: move |_| show_palette.set(false),
                                    IconClose { size: 12 }
                                }
                            }
                            div {
                                class: "palette-grid",
                                for preset in LIGHTING_PRESETS.iter() {
                                    div {
                                        key: "{preset.name}",
                                        class: "preset-color-item",
                                        style: "--item-color: {preset.primary};",
                                        onclick: {
                                            let primary = preset.primary;
                                            let accent = preset.accent;
                                            let rgb = preset.rgb;
                                            move |_| {
                                                apply_lighting_to_dom(primary, accent, rgb);
                                                show_palette.set(false);
                                            }
                                        },
                                        div { class: "preset-dot" }
                                        span { "{preset.name}" }
                                    }
                                }
                            }
                            div {
                                class: "custom-color-row",
                                span { "Custom Hue" }
                                input {
                                    r#type: "color",
                                    class: "custom-color-input",
                                    value: "{custom_hex.read()}",
                                    oninput: move |evt| {
                                        let val = evt.value();
                                        custom_hex.set(val.clone());
                                        let rgb = hex_to_rgb_str(&val);
                                        apply_lighting_to_dom(&val, &val, &rgb);
                                    }
                                }
                            }
                        }
                    }
                }

                // Dark/Light Theme Toggle
                button {
                    class: "icon-btn",
                    title: if *is_dark.read() { "Switch to Light Mode" } else { "Switch to Dark Mode" },
                    onclick: toggle_theme,
                    if *is_dark.read() {
                        IconSun { size: 16 }
                    } else {
                        IconMoon { size: 16 }
                    }
                }

                // Auth / User Profile
                if let Some(user) = user_opt {
                    div {
                        class: "flex items-center gap-2",
                        div {
                            class: "glass-badge badge-glow user-badge",
                            IconUser { size: 14 }
                            span { "{user.username}" }
                            if user.is_token_login {
                                span {
                                    class: "badge badge-warning text-xs font-mono ml-1",
                                    style: "font-size: 0.65rem; padding: 1px 6px; border-radius: 6px; background: rgba(245, 158, 11, 0.15); color: #fbbf24; border: 1px solid rgba(245, 158, 11, 0.3);",
                                    "token"
                                }
                            } else if user.is_admin {
                                span { class: "status-dot status-dot-pulse", style: "margin-left: 2px;" }
                            }
                        }
                        button {
                            class: "glass-btn btn-secondary btn-sm",
                            onclick: move |_| on_logout_click.call(()),
                            "Sign Out"
                        }
                    }
                } else {
                    div {
                        class: "flex items-center gap-2",
                        button {
                            class: "glass-btn btn-primary btn-sm",
                            onclick: move |_| on_login_click.call(()),
                            "Sign In"
                        }
                    }
                }
            }
        }
    }
}
