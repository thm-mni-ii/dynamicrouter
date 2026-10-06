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
pub fn GlassAutocompleteInput(
    value: Signal<String>,
    #[props(default)] suggestions: Vec<String>,
    #[props(default = "")] placeholder: &'static str,
    #[props(default = false)] disabled: bool,
    #[props(default = true)] allow_custom: bool,
    #[props(default = "")] class: &'static str,
    #[props(default = "")] style: &'static str,
    #[props(default)] on_change: Option<EventHandler<String>>,
) -> Element {
    let input_id = use_hook(|| {
        let rand_val = js_sys::Math::random().to_string().replace('.', "");
        format!("auto-input-{rand_val}")
    });

    let mut is_focused = use_signal(|| false);
    let mut is_hovering_dropdown = use_signal(|| false);
    let mut highlighted_idx = use_signal(|| Option::<usize>::None);

    let current_val = value.read().clone();
    let query = current_val.trim().to_lowercase();

    // Filter suggestions
    let filtered_suggestions: Vec<String> = suggestions
        .iter()
        .filter(|s| {
            if query.is_empty() || query == "*" {
                true
            } else {
                s.to_lowercase().contains(&query)
            }
        })
        .take(15)
        .cloned()
        .collect();

    let show_dropdown = (*is_focused.read() || *is_hovering_dropdown.read())
        && !filtered_suggestions.is_empty();

    let select_item = {
        let input_id = input_id.clone();
        move |item: &str| {
            value.set(item.to_string());
            if let Some(ref handler) = on_change {
                handler.call(item.to_string());
            }
            is_focused.set(false);
            is_hovering_dropdown.set(false);
            highlighted_idx.set(None);
            focus_input(&input_id);
        }
    };

    rsx! {
        div {
            class: "glass-autocomplete-wrapper",
            style: "{style}",

            input {
                id: "{input_id}",
                r#type: "text",
                class: "glass-input {class}",
                placeholder: "{placeholder}",
                value: "{value.read()}",
                disabled: disabled,
                onfocus: move |_| {
                    is_focused.set(true);
                    highlighted_idx.set(None);
                },
                onblur: move |_| {
                    if !*is_hovering_dropdown.read() {
                        is_focused.set(false);
                        highlighted_idx.set(None);
                    }
                },
                oninput: move |evt: FormEvent| {
                    let val = evt.value();
                    value.set(val.clone());
                    if let Some(ref handler) = on_change {
                        handler.call(val);
                    }
                    highlighted_idx.set(None);
                },
                onkeydown: {
                    let filtered = filtered_suggestions.clone();
                    let mut select = select_item.clone();
                    move |evt: KeyboardEvent| {
                        let cur_h_idx = *highlighted_idx.read();
                        match evt.key() {
                            Key::Enter | Key::Tab => {
                                if let Some(h_idx) = cur_h_idx {
                                    if let Some(chosen) = filtered.get(h_idx) {
                                        select(chosen);
                                    }
                                }
                            }
                            Key::ArrowDown => {
                                if !filtered.is_empty() {
                                    let next = match cur_h_idx {
                                        None => 0,
                                        Some(i) => if i + 1 < filtered.len() { i + 1 } else { 0 },
                                    };
                                    highlighted_idx.set(Some(next));
                                }
                            }
                            Key::ArrowUp => {
                                if !filtered.is_empty() {
                                    let prev = match cur_h_idx {
                                        None => filtered.len() - 1,
                                        Some(0) => filtered.len() - 1,
                                        Some(i) => i - 1,
                                    };
                                    highlighted_idx.set(Some(prev));
                                }
                            }
                            Key::Escape => {
                                is_focused.set(false);
                                is_hovering_dropdown.set(false);
                                highlighted_idx.set(None);
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Autocomplete Dropdown
            if show_dropdown {
                div {
                    class: "glass-autocomplete-dropdown",
                    onmouseenter: move |_| is_hovering_dropdown.set(true),
                    onmouseleave: move |_| is_hovering_dropdown.set(false),

                    for (idx, suggestion) in filtered_suggestions.iter().enumerate() {
                        {
                            let is_active = *highlighted_idx.read() == Some(idx);
                            let sug_clone = suggestion.clone();
                            let mut select_md = select_item.clone();
                            let mut select_ck = select_item.clone();
                            rsx! {
                                div {
                                    key: "{suggestion}",
                                    class: if is_active { "autocomplete-option active" } else { "autocomplete-option" },
                                    onmousedown: {
                                        let s = sug_clone.clone();
                                        move |evt: MouseEvent| {
                                            evt.prevent_default();
                                            select_md(&s);
                                        }
                                    },
                                    onclick: {
                                        let s = sug_clone.clone();
                                        move |evt: MouseEvent| {
                                            evt.stop_propagation();
                                            select_ck(&s);
                                        }
                                    },
                                    span { class: "autocomplete-option-label", "{suggestion}" }
                                    span { class: "autocomplete-option-badge", "select" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
