use crate::components::icons::IconClose;
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

fn add_chip_value(
    mut values: Signal<Vec<String>>,
    mut input_text: Signal<String>,
    raw: &str,
    max_items: Option<usize>,
) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Some(1) = max_items {
        values.set(vec![trimmed.to_string()]);
    } else {
        let exists = values.read().iter().any(|v| v.eq_ignore_ascii_case(trimmed));
        if !exists {
            if let Some(max) = max_items {
                if values.read().len() < max {
                    values.write().push(trimmed.to_string());
                }
            } else {
                values.write().push(trimmed.to_string());
            }
        }
    }
    input_text.set(String::new());
}

#[component]
pub fn ChipInput(
    values: Signal<Vec<String>>,
    #[props(default)] suggestions: Vec<String>,
    #[props(default = "")] placeholder: &'static str,
    #[props(default = false)] disabled: bool,
    #[props(default = true)] allow_custom: bool,
    #[props(default)] max_items: Option<usize>,
    #[props(default = "")] class: &'static str,
    #[props(default = "")] style: &'static str,
) -> Element {
    let input_id = use_hook(|| {
        let rand_val = js_sys::Math::random().to_string().replace('.', "");
        format!("chip-input-{rand_val}")
    });

    let mut input_text = use_signal(String::new);
    let mut is_focused = use_signal(|| false);
    let mut is_hovering_dropdown = use_signal(|| false);
    let mut highlighted_idx = use_signal(|| Option::<usize>::None);

    let current_values = values.read().clone();
    let query = input_text.read().trim().to_lowercase();

    // Filter suggestions: exclude already selected items, match query
    let filtered_suggestions: Vec<String> = suggestions
        .iter()
        .filter(|s| {
            if max_items == Some(1) {
                true
            } else {
                !current_values.iter().any(|v| v.eq_ignore_ascii_case(s.as_str()))
            }
        })
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

    let mut remove_chip = move |idx: usize| {
        if !disabled {
            let mut list = values.read().clone();
            if idx < list.len() {
                list.remove(idx);
                values.set(list);
            }
        }
    };

    let add_item = {
        let input_id = input_id.clone();
        move |item: &str| {
            add_chip_value(values, input_text, item, max_items);
            highlighted_idx.set(None);
            is_focused.set(true);
            focus_input(&input_id);
        }
    };

    let display_placeholder = if current_values.is_empty() {
        placeholder
    } else {
        ""
    };

    let input_id_clone = input_id.clone();

    rsx! {
        div {
            class: "glass-chip-input-wrapper {class}",
            style: "{style}",

            div {
                class: if disabled { "glass-chip-input-container disabled" } else { "glass-chip-input-container" },
                onclick: move |_| {
                    if !disabled {
                        focus_input(&input_id_clone);
                    }
                },

                // Selected chips
                for (idx, item) in current_values.iter().enumerate() {
                    div {
                        key: "{item}_{idx}",
                        class: "glass-chip-item",
                        span { class: "chip-label", "{item}" }
                        if !disabled {
                            button {
                                r#type: "button",
                                class: "chip-remove-btn",
                                title: "Remove {item}",
                                onclick: move |evt| {
                                    evt.stop_propagation();
                                    remove_chip(idx);
                                },
                                IconClose { size: 12 }
                            }
                        }
                    }
                }

                // Inline text input
                if !disabled {
                    input {
                        id: "{input_id}",
                        r#type: "text",
                        class: "chip-text-input font-mono",
                        placeholder: "{display_placeholder}",
                        value: "{input_text.read()}",
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
                            if val.contains(',') || val.contains('\n') {
                                for part in val.split(|c| c == ',' || c == '\n') {
                                    add_chip_value(values, input_text, part, max_items);
                                }
                            } else {
                                input_text.set(val);
                                highlighted_idx.set(None);
                            }
                        },
                        onkeydown: {
                            let filtered = filtered_suggestions.clone();
                            let mut add = add_item.clone();
                            move |evt: KeyboardEvent| {
                                let cur_h_idx = *highlighted_idx.read();
                                match evt.key() {
                                    Key::Enter | Key::Tab => {
                                        if let Some(h_idx) = cur_h_idx {
                                            if let Some(chosen) = filtered.get(h_idx) {
                                                add(chosen);
                                                return;
                                            }
                                        }
                                        let cur = input_text.read().trim().to_string();
                                        if !cur.is_empty() {
                                            if let Some(first_match) = filtered.first() {
                                                if first_match.eq_ignore_ascii_case(&cur) {
                                                    add(first_match);
                                                    return;
                                                }
                                            }
                                            if allow_custom {
                                                add(&cur);
                                            }
                                        }
                                    }
                                    Key::Backspace => {
                                        if input_text.read().is_empty() && !values.read().is_empty() {
                                            values.write().pop();
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
                            let mut add_md = add_item.clone();
                            let mut add_ck = add_item.clone();
                            rsx! {
                                div {
                                    key: "{suggestion}",
                                    class: if is_active { "autocomplete-option active" } else { "autocomplete-option" },
                                    onmousedown: {
                                        let s = sug_clone.clone();
                                        move |evt: MouseEvent| {
                                            evt.prevent_default();
                                            add_md(&s);
                                        }
                                    },
                                    onclick: {
                                        let s = sug_clone.clone();
                                        move |evt: MouseEvent| {
                                            evt.stop_propagation();
                                            add_ck(&s);
                                        }
                                    },
                                    span { class: "autocomplete-option-label", "{suggestion}" }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
