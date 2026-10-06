use crate::components::icons::*;
use crate::state::Toast;
use dioxus::prelude::*;
use web_sys::window;

#[component]
pub fn GlassCard(
    #[props(default = "")] class: &'static str,
    #[props(default = "")] style: &'static str,
    #[props(default = false)] glow: bool,
    children: Element,
) -> Element {
    let glow_class = if glow { "glass-card-glow" } else { "" };
    rsx! {
        div {
            class: "glass-card {glow_class} {class}",
            style: "{style}",
            {children}
        }
    }
}

#[component]
pub fn GlassBadge(
    #[props(default = "")] variant: &'static str, // "glow", "success", "warning", "danger"
    #[props(default = false)] pulse: bool,
    children: Element,
) -> Element {
    let variant_class = match variant {
        "glow" => "badge-glow",
        "success" => "badge-success",
        "warning" => "badge-warning",
        "danger" => "badge-danger",
        _ => "",
    };
    let dot_pulse = if pulse { "status-dot-pulse" } else { "" };

    rsx! {
        span {
            class: "glass-badge {variant_class}",
            if pulse {
                span { class: "status-dot {dot_pulse}" }
            }
            {children}
        }
    }
}

#[component]
pub fn GlassModal(
    title: String,
    on_close: EventHandler<()>,
    children: Element,
) -> Element {
    rsx! {
        div {
            class: "modal-overlay",
            onclick: move |_e| {
                on_close.call(());
            },
            div {
                class: "glass-modal",
                onclick: move |e| {
                    e.stop_propagation();
                },
                div {
                    class: "modal-header",
                    h3 { class: "modal-title", "{title}" }
                    button {
                        class: "icon-btn",
                        onclick: move |_| on_close.call(()),
                        IconClose { size: 16 }
                    }
                }
                div {
                    class: "modal-body",
                    {children}
                }
            }
        }
    }
}

#[component]
pub fn CodeBlock(
    title: Option<String>,
    code: String,
) -> Element {
    let mut copied = use_signal(|| false);

    let copy_code = {
        let code = code.clone();
        move |_| {
            if let Some(w) = window() {
                let nav = w.navigator();
                let clip = nav.clipboard();
                let _ = clip.write_text(&code);
                copied.set(true);
            }
        }
    };

    let title_display = title.as_deref().unwrap_or("JSON Payload").to_string();

    rsx! {
        div {
            class: "code-block-wrapper",
            div {
                class: "code-block-header",
                span { "{title_display}" }
                button {
                    class: "glass-btn btn-secondary btn-sm",
                    onclick: copy_code,
                    if *copied.read() {
                        IconCheck { size: 14 }
                        span { "Copied!" }
                    } else {
                        span { "Copy" }
                    }
                }
            }
            pre {
                class: "code-pre",
                code { "{code}" }
            }
        }
    }
}

#[component]
pub fn ToastContainer(
    toasts: Signal<Vec<Toast>>,
) -> Element {
    rsx! {
        div {
            class: "toast-container",
            for toast in toasts.read().iter() {
                div {
                    key: "{toast.id}",
                    class: if toast.is_error { "toast toast-error" } else { "toast toast-success" },
                    span {
                        class: "toast-icon",
                        if toast.is_error {
                            IconAlertTriangle { size: 16 }
                        } else {
                            IconCheck { size: 16 }
                        }
                    }
                    span { "{toast.message}" }
                }
            }
        }
    }
}
