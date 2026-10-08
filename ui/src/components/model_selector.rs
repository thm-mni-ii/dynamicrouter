use crate::components::icons::*;
use crate::components::GlassBadge;
use crate::models::UserModelStatus;
use dioxus::prelude::*;
use wasm_bindgen::JsCast;

fn focus_input(id: &str) {
    if let Some(w) = web_sys::window() {
        if let Some(doc) = w.document() {
            if let Some(el) = doc.get_element_by_id(id) {
                if let Ok(input_el) = el.dyn_into::<web_sys::HtmlInputElement>() {
                    let _ = input_el.focus();
                }
            }
        }
    }
}

#[component]
pub fn ModelSelector(
    models: Signal<Vec<UserModelStatus>>,
    selected_model: Signal<String>,
    #[props(default = false)] disabled: bool,
) -> Element {
    let mut is_open = use_signal(|| false);
    let mut search_query = use_signal(String::new);
    let mut status_filter = use_signal(|| "all".to_string());
    let mut highlighted_idx = use_signal(|| Option::<usize>::None);

    let search_input_id = use_hook(|| {
        let rand_val = js_sys::Math::random().to_string().replace('.', "");
        format!("model-search-{rand_val}")
    });

    let current_selected_id = selected_model.read().clone();
    let all_models = models.read().clone();

    let active_model = all_models
        .iter()
        .find(|m| m.id == current_selected_id)
        .cloned();

    // Stats for filter badges
    let total_count = all_models.len();
    let ready_count = all_models.iter().filter(|m| m.status == "running").count();
    let upstream_count = all_models.iter().filter(|m| m.status == "upstream").count();
    let cold_count = total_count.saturating_sub(ready_count + upstream_count);

    // Filter models by search query and status filter
    let q = search_query.read().trim().to_lowercase();
    let s_filter = status_filter.read().clone();

    let filtered_models: Vec<UserModelStatus> = all_models
        .into_iter()
        .filter(|m| {
            // Status filter check
            let matches_status = match s_filter.as_str() {
                "running" => m.status == "running",
                "upstream" => m.status == "upstream",
                "cold" => m.status != "running" && m.status != "upstream",
                _ => true,
            };
            if !matches_status {
                return false;
            }

            // Search query check
            if q.is_empty() {
                return true;
            }
            if m.name.to_lowercase().contains(&q) || m.id.to_lowercase().contains(&q) {
                return true;
            }
            m.aliases.iter().any(|a| a.to_lowercase().contains(&q))
        })
        .collect();

    let do_toggle = {
        let input_id = search_input_id.clone();
        move |()| {
            if disabled {
                return;
            }
            let next = !*is_open.read();
            is_open.set(next);
            highlighted_idx.set(None);
            if next {
                // Focus search input after opening
                let id = input_id.clone();
                spawn(async move {
                    gloo_timers::future::TimeoutFuture::new(50).await;
                    focus_input(&id);
                });
            }
        }
    };

    let select_model = move |id: String| {
        selected_model.set(id);
        is_open.set(false);
        search_query.set(String::new());
        highlighted_idx.set(None);
    };

    rsx! {
        div {
            class: "model-selector-wrapper",

            // Trigger Button
            div {
                class: if *is_open.read() {
                    "model-selector-trigger active"
                } else if disabled {
                    "model-selector-trigger disabled"
                } else {
                    "model-selector-trigger"
                },
                tabindex: if disabled { -1 } else { 0 },
                onclick: {
                    let mut toggle = do_toggle.clone();
                    move |_| toggle(())
                },
                onkeydown: {
                    let mut toggle = do_toggle.clone();
                    move |evt: KeyboardEvent| {
                        match evt.key() {
                            Key::Enter | Key::ArrowDown => {
                                evt.prevent_default();
                                toggle(());
                            }
                            Key::Escape => {
                                is_open.set(false);
                            }
                            _ => {}
                        }
                    }
                },

                // Active Model Display
                div {
                    class: "model-selector-trigger-content",
                    if let Some(ref active) = active_model {
                        div {
                            class: "flex items-center gap-2 overflow-hidden",
                            if active.status == "running" {
                                span { class: "status-dot status-dot-running", title: "Ready" }
                            } else if active.status == "upstream" {
                                span { class: "status-dot status-dot-upstream", title: "Cloud API" }
                            } else {
                                span { class: "status-dot status-dot-cold", title: "Cold Start" }
                            }
                            div {
                                class: "flex flex-col overflow-hidden text-left",
                                span { class: "model-selector-name", "{active.name}" }
                                span { class: "model-selector-id", "{active.id}" }
                            }
                        }
                    } else if models.read().is_empty() {
                        span { class: "text-muted text-sm italic", "Loading models..." }
                    } else {
                        span { class: "text-muted text-sm", "Select a model..." }
                    }
                }

                // Status Badge & Chevron Icon
                div {
                    class: "flex items-center gap-2 flex-shrink-0 ml-2",
                    if let Some(ref active) = active_model {
                        if active.status == "running" {
                            GlassBadge { variant: "glow", "{active.active_instances} active" }
                        } else if active.status == "upstream" {
                            GlassBadge { variant: "primary", "🟣 Cloud" }
                        } else {
                            GlassBadge { variant: "secondary", "⚪ Cold" }
                        }
                    }
                    div {
                        class: if *is_open.read() {
                            "model-selector-chevron open"
                        } else {
                            "model-selector-chevron"
                        },
                        IconChevronDown { size: 16 }
                    }
                }
            }

            // Click outside backdrop
            if *is_open.read() {
                div {
                    class: "model-selector-backdrop",
                    onclick: move |_| {
                        is_open.set(false);
                        highlighted_idx.set(None);
                    },
                }
            }

            // Dropdown Popover
            if *is_open.read() {
                div {
                    class: "model-selector-dropdown",
                    onclick: move |e| e.stop_propagation(),

                    // Search Header
                    div {
                        class: "model-selector-search-box",
                        IconSearch { size: 14, class: "text-muted" }
                        input {
                            id: "{search_input_id}",
                            r#type: "text",
                            class: "model-selector-search-input",
                            placeholder: "Search models by name, ID, or alias...",
                            value: "{search_query.read()}",
                            oninput: move |evt: FormEvent| {
                                search_query.set(evt.value());
                                highlighted_idx.set(None);
                            },
                            onkeydown: {
                                let filtered = filtered_models.clone();
                                let mut select = select_model.clone();
                                move |evt: KeyboardEvent| {
                                    let cur_h = *highlighted_idx.read();
                                    match evt.key() {
                                        Key::ArrowDown => {
                                            evt.prevent_default();
                                            if !filtered.is_empty() {
                                                let next = match cur_h {
                                                    None => 0,
                                                    Some(i) => if i + 1 < filtered.len() { i + 1 } else { 0 },
                                                };
                                                highlighted_idx.set(Some(next));
                                            }
                                        }
                                        Key::ArrowUp => {
                                            evt.prevent_default();
                                            if !filtered.is_empty() {
                                                let prev = match cur_h {
                                                    None => filtered.len() - 1,
                                                    Some(0) => filtered.len() - 1,
                                                    Some(i) => i - 1,
                                                };
                                                highlighted_idx.set(Some(prev));
                                            }
                                        }
                                        Key::Enter => {
                                            evt.prevent_default();
                                            if let Some(idx) = cur_h {
                                                if let Some(chosen) = filtered.get(idx) {
                                                    select(chosen.id.clone());
                                                }
                                            } else if let Some(first) = filtered.first() {
                                                select(first.id.clone());
                                            }
                                        }
                                        Key::Escape => {
                                            is_open.set(false);
                                        }
                                        _ => {}
                                    }
                                }
                            },
                        }
                        if !search_query.read().is_empty() {
                            button {
                                r#type: "button",
                                class: "model-selector-clear-btn",
                                onclick: move |_| {
                                    search_query.set(String::new());
                                    highlighted_idx.set(None);
                                },
                                IconClose { size: 12 }
                            }
                        }
                    }

                    // Status Filter Tabs
                    div {
                        class: "model-selector-filter-pills",
                        button {
                            r#type: "button",
                            class: if *status_filter.read() == "all" {
                                "selector-pill active"
                            } else {
                                "selector-pill"
                            },
                            onclick: move |_| status_filter.set("all".to_string()),
                            "All ({total_count})"
                        }
                        if ready_count > 0 {
                            button {
                                r#type: "button",
                                class: if *status_filter.read() == "running" {
                                    "selector-pill active"
                                } else {
                                    "selector-pill"
                                },
                                onclick: move |_| status_filter.set("running".to_string()),
                                "🟢 Ready ({ready_count})"
                            }
                        }
                        if upstream_count > 0 {
                            button {
                                r#type: "button",
                                class: if *status_filter.read() == "upstream" {
                                    "selector-pill active"
                                } else {
                                    "selector-pill"
                                },
                                onclick: move |_| status_filter.set("upstream".to_string()),
                                "🟣 Cloud ({upstream_count})"
                            }
                        }
                        if cold_count > 0 {
                            button {
                                r#type: "button",
                                class: if *status_filter.read() == "cold" {
                                    "selector-pill active"
                                } else {
                                    "selector-pill"
                                },
                                onclick: move |_| status_filter.set("cold".to_string()),
                                "⚪ Cold ({cold_count})"
                            }
                        }
                    }

                    // Options List
                    div {
                        class: "model-selector-options-list",
                        if filtered_models.is_empty() {
                            div {
                                class: "model-selector-empty",
                                p { "No models found matching your search." }
                            }
                        } else {
                            for (idx, m) in filtered_models.iter().enumerate() {
                                {
                                    let is_selected = m.id == current_selected_id;
                                    let is_highlighted = *highlighted_idx.read() == Some(idx);
                                    let mid = m.id.clone();
                                    let mut select = select_model.clone();

                                    rsx! {
                                        div {
                                            key: "{m.id}",
                                            class: if is_selected && is_highlighted {
                                                "model-selector-option selected highlighted"
                                            } else if is_selected {
                                                "model-selector-option selected"
                                            } else if is_highlighted {
                                                "model-selector-option highlighted"
                                            } else {
                                                "model-selector-option"
                                            },
                                            onclick: move |_| select(mid.clone()),
                                            onmouseenter: move |_| highlighted_idx.set(Some(idx)),

                                            // Left: Model Identity & Metadata
                                            div {
                                                class: "model-option-main",
                                                div {
                                                    class: "flex items-center gap-2",
                                                    if m.status == "running" {
                                                        span { class: "status-dot status-dot-running" }
                                                    } else if m.status == "upstream" {
                                                        span { class: "status-dot status-dot-upstream" }
                                                    } else {
                                                        span { class: "status-dot status-dot-cold" }
                                                    }
                                                    span { class: "model-option-name", "{m.name}" }
                                                }
                                                span { class: "model-option-id", "{m.id}" }

                                                // Context Length & Aliases badges
                                                if m.ctx_length.is_some() || !m.aliases.is_empty() {
                                                    div {
                                                        class: "flex items-center gap-1.5 flex-wrap mt-0.5",
                                                        if let Some(ctx) = m.ctx_length {
                                                            span {
                                                                class: "model-option-meta-badge",
                                                                if ctx >= 1000 {
                                                                    "{ctx / 1000}k ctx"
                                                                } else {
                                                                    "{ctx} ctx"
                                                                }
                                                            }
                                                        }
                                                        for alias in m.aliases.iter() {
                                                            span {
                                                                key: "{alias}",
                                                                class: "model-option-meta-badge",
                                                                "alias: {alias}"
                                                            }
                                                        }
                                                    }
                                                }
                                            }

                                            // Right: Status Badge & Checkmark
                                            div {
                                                class: "model-option-status",
                                                if m.status == "running" {
                                                    GlassBadge { variant: "glow", "🟢 Ready" }
                                                } else if m.status == "upstream" {
                                                    GlassBadge { variant: "primary", "🟣 Cloud API" }
                                                } else {
                                                    GlassBadge { variant: "secondary", "⚪ Cold Start" }
                                                }
                                                if is_selected {
                                                    div {
                                                        class: "model-option-check",
                                                        IconCheck { size: 16 }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
