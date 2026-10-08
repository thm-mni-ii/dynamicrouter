use dioxus::prelude::*;

#[component]
pub fn IconBolt(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polygon { points: "13 2 3 14 12 14 11 22 21 10 12 10 13 2" }
        }
    }
}

#[component]
pub fn IconChat(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" }
        }
    }
}

#[component]
pub fn IconKey(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "7.5", cy: "15.5", r: "5.5" }
            path { d: "m21 2-9.6 9.6" }
            path { d: "m15.5 7.5 3 3L22 7l-3-3" }
        }
    }
}

#[component]
pub fn IconLogs(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" }
            rect { x: "8", y: "2", width: "8", height: "4", rx: "1", ry: "1" }
            path { d: "M9 12h6" }
            path { d: "M9 16h6" }
        }
    }
}

#[component]
pub fn IconMetrics(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "18", y1: "20", x2: "18", y2: "10" }
            line { x1: "12", y1: "20", x2: "12", y2: "4" }
            line { x1: "6", y1: "20", x2: "6", y2: "14" }
        }
    }
}

#[component]
pub fn IconDashboard(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "3", y: "3", width: "7", height: "9", rx: "1" }
            rect { x: "14", y: "3", width: "7", height: "5", rx: "1" }
            rect { x: "14", y: "12", width: "7", height: "9", rx: "1" }
            rect { x: "3", y: "16", width: "7", height: "5", rx: "1" }
        }
    }
}

#[component]
pub fn IconBot(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "4", y: "8", width: "16", height: "12", rx: "2" }
            path { d: "M2 14h2" }
            path { d: "M20 14h2" }
            path { d: "M15 13v2" }
            path { d: "M9 13v2" }
            path { d: "M12 2v6" }
            circle { cx: "12", cy: "2", r: "1" }
        }
    }
}

#[component]
pub fn IconGlobe(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "10" }
            line { x1: "2", y1: "12", x2: "22", y2: "12" }
            path { d: "M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z" }
        }
    }
}

#[component]
pub fn IconServer(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "2", y: "2", width: "20", height: "8", rx: "2", ry: "2" }
            rect { x: "2", y: "14", width: "20", height: "8", rx: "2", ry: "2" }
            line { x1: "6", y1: "6", x2: "6.01", y2: "6" }
            line { x1: "6", y1: "18", x2: "6.01", y2: "18" }
        }
    }
}

#[component]
pub fn IconUsers(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
            circle { cx: "9", cy: "7", r: "4" }
            path { d: "M22 21v-2a4 4 0 0 0-3-3.87" }
            path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
        }
    }
}

#[component]
pub fn IconGpu(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "4", y: "4", width: "16", height: "16", rx: "2" }
            rect { x: "9", y: "9", width: "6", height: "6" }
            path { d: "M15 2v2" }
            path { d: "M15 20v2" }
            path { d: "M2 15h2" }
            path { d: "M2 9h2" }
            path { d: "M20 15h2" }
            path { d: "M20 9h2" }
            path { d: "M9 2v2" }
            path { d: "M9 20v2" }
        }
    }
}

#[component]
pub fn IconMonitor(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            rect { x: "2", y: "3", width: "20", height: "14", rx: "2", ry: "2" }
            line { x1: "8", y1: "21", x2: "16", y2: "21" }
            line { x1: "12", y1: "17", x2: "12", y2: "21" }
        }
    }
}

#[component]
pub fn IconShuffle(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "16 3 21 3 21 8" }
            line { x1: "4", y1: "20", x2: "21", y2: "3" }
            polyline { points: "21 16 21 21 16 21" }
            line { x1: "15", y1: "15", x2: "21", y2: "21" }
            line { x1: "4", y1: "4", x2: "9", y2: "9" }
        }
    }
}

#[component]
pub fn IconAudit(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z" }
            polyline { points: "14 2 14 8 20 8" }
            line { x1: "16", y1: "13", x2: "8", y2: "13" }
            line { x1: "16", y1: "17", x2: "8", y2: "17" }
            polyline { points: "10 9 9 9 8 9" }
        }
    }
}

#[component]
pub fn IconSun(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "5" }
            line { x1: "12", y1: "1", x2: "12", y2: "3" }
            line { x1: "12", y1: "21", x2: "12", y2: "23" }
            line { x1: "4.22", y1: "4.22", x2: "5.64", y2: "5.64" }
            line { x1: "18.36", y1: "18.36", x2: "19.78", y2: "19.78" }
            line { x1: "1", y1: "12", x2: "3", y2: "12" }
            line { x1: "21", y1: "12", x2: "23", y2: "12" }
            line { x1: "4.22", y1: "19.78", x2: "5.64", y2: "18.36" }
            line { x1: "18.36", y1: "5.64", x2: "19.78", y2: "4.22" }
        }
    }
}

#[component]
pub fn IconMoon(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" }
        }
    }
}

#[component]
pub fn IconUser(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M19 21v-2a4 4 0 0 0-4-4H9a4 4 0 0 0-4 4v2" }
            circle { cx: "12", cy: "7", r: "4" }
        }
    }
}

#[component]
pub fn IconRefresh(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "23 4 23 10 17 10" }
            polyline { points: "1 20 1 14 7 14" }
            path { d: "M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" }
        }
    }
}

#[component]
pub fn IconTrash(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "3 6 5 6 21 6" }
            path { d: "M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2" }
            line { x1: "10", y1: "11", x2: "10", y2: "17" }
            line { x1: "14", y1: "11", x2: "14", y2: "17" }
        }
    }
}

#[component]
pub fn IconClose(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "18", y1: "6", x2: "6", y2: "18" }
            line { x1: "6", y1: "6", x2: "18", y2: "18" }
        }
    }
}

#[component]
pub fn IconCheck(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "20 6 9 17 4 12" }
        }
    }
}

#[component]
pub fn IconAlertTriangle(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3Z" }
            line { x1: "12", y1: "9", x2: "12", y2: "13" }
            line { x1: "12", y1: "17", x2: "12.01", y2: "17" }
        }
    }
}

#[component]
pub fn IconPlus(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "12", y1: "5", x2: "12", y2: "19" }
            line { x1: "5", y1: "12", x2: "19", y2: "12" }
        }
    }
}

#[component]
pub fn IconSend(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "22", y1: "2", x2: "11", y2: "13" }
            polygon { points: "22 2 15 22 11 13 2 9 22 2" }
        }
    }
}

#[component]
pub fn IconChevronDown(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            polyline { points: "6 9 12 15 18 9" }
        }
    }
}

#[component]
pub fn IconSliders(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            line { x1: "4", y1: "21", x2: "4", y2: "14" }
            line { x1: "4", y1: "10", x2: "4", y2: "3" }
            line { x1: "12", y1: "21", x2: "12", y2: "12" }
            line { x1: "12", y1: "8", x2: "12", y2: "3" }
            line { x1: "20", y1: "21", x2: "20", y2: "16" }
            line { x1: "20", y1: "12", x2: "20", y2: "3" }
            line { x1: "1", y1: "14", x2: "7", y2: "14" }
            line { x1: "9", y1: "8", x2: "15", y2: "8" }
            line { x1: "17", y1: "16", x2: "23", y2: "16" }
        }
    }
}

#[component]
pub fn IconShield(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z" }
        }
    }
}

#[component]
pub fn IconGauge(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            path { d: "m12 14 4-4" }
            path { d: "M3.34 19a10 10 0 1 1 17.32 0" }
        }
    }
}

#[component]
pub fn IconClock(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "12", cy: "12", r: "10" }
            polyline { points: "12 6 12 12 16 14" }
        }
    }
}

#[component]
pub fn IconSearch(#[props(default = 16)] size: u32, #[props(into, default)] class: String) -> Element {
    rsx! {
        svg {
            class: "icon {class}",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            circle { cx: "11", cy: "11", r: "8" }
            line { x1: "21", y1: "21", x2: "16.65", y2: "16.65" }
        }
    }
}
