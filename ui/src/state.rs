use serde::{Deserialize, Serialize};
use std::fmt;
use wasm_bindgen::JsCast;
use web_sys::window;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActiveTab {
    // User Tabs
    Chat,
    Tokens,
    UserLogs,
    UserMetrics,

    // Admin Tabs
    AdminDashboard,
    AdminModels,
    AdminOpenAiApis,
    AdminBackends,
    AdminUsers,
    AdminFactoryGpus,
    AdminInstances,
    AdminAliases,
    AdminSystemLogs,
}

impl ActiveTab {
    pub fn is_admin(&self) -> bool {
        matches!(
            self,
            Self::AdminDashboard
                | Self::AdminModels
                | Self::AdminOpenAiApis
                | Self::AdminBackends
                | Self::AdminUsers
                | Self::AdminFactoryGpus
                | Self::AdminInstances
                | Self::AdminAliases
                | Self::AdminSystemLogs
        )
    }
}

impl fmt::Display for ActiveTab {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chat => write!(f, "Chat Playground"),
            Self::Tokens => write!(f, "API Tokens"),
            Self::UserLogs => write!(f, "My Logs"),
            Self::UserMetrics => write!(f, "My Metrics"),
            Self::AdminDashboard => write!(f, "Dashboard"),
            Self::AdminModels => write!(f, "Models"),
            Self::AdminOpenAiApis => write!(f, "OpenAI APIs"),
            Self::AdminBackends => write!(f, "Backends"),
            Self::AdminUsers => write!(f, "Users"),
            Self::AdminFactoryGpus => write!(f, "Factory GPUs"),
            Self::AdminInstances => write!(f, "Instances"),
            Self::AdminAliases => write!(f, "Aliases"),
            Self::AdminSystemLogs => write!(f, "Audit Logs"),
        }
    }
}

// --------------------------------------------------------------------------
// Routing / URL Synchronization
// --------------------------------------------------------------------------
pub fn tab_from_path(path: &str) -> ActiveTab {
    let clean = path.trim_matches('/');
    match clean {
        "" | "chat" => ActiveTab::Chat,
        "tokens" => ActiveTab::Tokens,
        "logs" | "user-logs" | "user_logs" => ActiveTab::UserLogs,
        "metrics" | "user-metrics" | "user_metrics" => ActiveTab::UserMetrics,
        "admin" | "admin/dashboard" => ActiveTab::AdminDashboard,
        "admin/models" => ActiveTab::AdminModels,
        "admin/openai-apis" | "admin/openai_apis" | "admin/openai" => ActiveTab::AdminOpenAiApis,
        "admin/backends" => ActiveTab::AdminBackends,
        "admin/users" => ActiveTab::AdminUsers,
        "admin/gpus" | "admin/factory-gpus" | "admin/factory_gpus" => ActiveTab::AdminFactoryGpus,
        "admin/instances" => ActiveTab::AdminInstances,
        "admin/aliases" => ActiveTab::AdminAliases,
        "admin/audit-logs" | "admin/system-logs" | "admin/system_logs" => ActiveTab::AdminSystemLogs,
        _ => ActiveTab::Chat,
    }
}

pub fn path_for_tab(tab: ActiveTab) -> &'static str {
    match tab {
        ActiveTab::Chat => "/chat",
        ActiveTab::Tokens => "/tokens",
        ActiveTab::UserLogs => "/logs",
        ActiveTab::UserMetrics => "/metrics",
        ActiveTab::AdminDashboard => "/admin/dashboard",
        ActiveTab::AdminModels => "/admin/models",
        ActiveTab::AdminOpenAiApis => "/admin/openai-apis",
        ActiveTab::AdminBackends => "/admin/backends",
        ActiveTab::AdminUsers => "/admin/users",
        ActiveTab::AdminFactoryGpus => "/admin/gpus",
        ActiveTab::AdminInstances => "/admin/instances",
        ActiveTab::AdminAliases => "/admin/aliases",
        ActiveTab::AdminSystemLogs => "/admin/audit-logs",
    }
}

pub fn get_initial_tab() -> ActiveTab {
    if let Some(w) = window() {
        if let Ok(pathname) = w.location().pathname() {
            return tab_from_path(&pathname);
        }
    }
    ActiveTab::Chat
}

pub fn set_browser_path_for_tab(tab: ActiveTab) {
    if let Some(w) = window() {
        let path = path_for_tab(tab);
        if let Ok(pathname) = w.location().pathname() {
            if pathname == path || (pathname == "/" && tab == ActiveTab::Chat) {
                return;
            }
        }
        if let Ok(history) = w.history() {
            let _ = history.push_state_with_url(&wasm_bindgen::JsValue::NULL, "", Some(path));
        }
    }
}

// --------------------------------------------------------------------------
// Lighting Preset Colors
// --------------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LightingPreset {
    pub name: &'static str,
    pub primary: &'static str,
    pub accent: &'static str,
    pub rgb: &'static str,
}

pub const LIGHTING_PRESETS: &[LightingPreset] = &[
    LightingPreset {
        name: "Cyan Aurora",
        primary: "#00f2fe",
        accent: "#4facfe",
        rgb: "0, 242, 254",
    },
    LightingPreset {
        name: "Electric Violet",
        primary: "#a855f7",
        accent: "#c084fc",
        rgb: "168, 85, 247",
    },
    LightingPreset {
        name: "Emerald Matrix",
        primary: "#10b981",
        accent: "#34d399",
        rgb: "16, 185, 129",
    },
    LightingPreset {
        name: "Sunset Blaze",
        primary: "#f97316",
        accent: "#fb923c",
        rgb: "249, 115, 22",
    },
    LightingPreset {
        name: "Crimson Neon",
        primary: "#f43f5e",
        accent: "#fb7185",
        rgb: "244, 63, 94",
    },
    LightingPreset {
        name: "Cyber Gold",
        primary: "#eab308",
        accent: "#fde047",
        rgb: "234, 179, 8",
    },
];

pub fn apply_lighting_to_dom(primary: &str, accent: &str, rgb: &str) {
    if let Some(doc) = window().and_then(|w| w.document()) {
        if let Some(el) = doc.document_element() {
            if let Ok(html_el) = el.dyn_into::<web_sys::HtmlElement>() {
                let style = html_el.style();
                let _ = style.set_property("--glow-primary", primary);
                let _ = style.set_property("--glow-accent", accent);
                let _ = style.set_property("--glow-rgb", rgb);
                let _ = style.set_property(
                    "--glow-gradient",
                    &format!("linear-gradient(135deg, {primary} 0%, {accent} 100%)"),
                );
            }
        }
    }
    if let Some(w) = window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = w.set_item("dr_glow_primary", primary);
        let _ = w.set_item("dr_glow_accent", accent);
        let _ = w.set_item("dr_glow_rgb", rgb);
    }
}

pub fn hex_to_rgb_str(hex: &str) -> String {
    let clean = hex.trim_start_matches('#');
    if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(0);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(242);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(254);
        format!("{r}, {g}, {b}")
    } else {
        "0, 242, 254".to_string()
    }
}

// --------------------------------------------------------------------------
// Theme (Dark / Light)
// --------------------------------------------------------------------------
pub fn apply_theme_to_dom(is_dark: bool) {
    let theme_str = if is_dark { "dark" } else { "light" };
    if let Some(doc) = window().and_then(|w| w.document()) {
        if let Some(el) = doc.document_element() {
            let _ = el.set_attribute("data-theme", theme_str);
        }
    }
    if let Some(w) = window().and_then(|w| w.local_storage().ok().flatten()) {
        let _ = w.set_item("dr_theme", theme_str);
    }
}

pub fn get_initial_theme() -> bool {
    window()
        .and_then(|w| w.local_storage().ok().flatten())
        .and_then(|s| s.get_item("dr_theme").ok().flatten())
        .map(|t| t == "dark")
        .unwrap_or(true)
}

// --------------------------------------------------------------------------
// Toast Messages
// --------------------------------------------------------------------------
#[derive(Clone, Debug, PartialEq)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub is_error: bool,
}
