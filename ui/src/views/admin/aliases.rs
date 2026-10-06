use crate::api::{
    delete_admin_alias, fetch_admin_aliases, save_admin_alias,
};
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard, GlassModal};
use crate::models::AliasRecord;
use crate::state::Toast;
use dioxus::prelude::*;

fn load_aliases_action(
    mut is_loading: Signal<bool>,
    mut aliases: Signal<Vec<AliasRecord>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_aliases().await {
            Ok(list) => aliases.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load aliases: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminAliasesView(toasts: Signal<Vec<Toast>>) -> Element {
    let aliases = use_signal(Vec::<AliasRecord>::new);
    let is_loading = use_signal(|| false);
    let mut editing_alias = use_signal(|| Option::<(AliasRecord, bool)>::None);

    let mut form_alias = use_signal(String::new);
    let mut form_target = use_signal(String::new);

    let _ = use_resource(move || async move {
        load_aliases_action(is_loading, aliases, toasts);
    });

    let open_create_modal = move |_| {
        form_alias.set(String::new());
        form_target.set(String::new());
        editing_alias.set(Some((
            AliasRecord {
                alias: String::new(),
                target: String::new(),
            },
            true,
        )));
    };

    let save_alias = move |_| {
        let (_, is_new) = match editing_alias.read().clone() {
            Some(a) => a,
            None => return,
        };

        let alias = form_alias.read().trim().to_string();
        let target = form_target.read().trim().to_string();
        if alias.is_empty() || target.is_empty() {
            return;
        }

        let payload = AliasRecord { alias, target };

        spawn(async move {
            match save_admin_alias(&payload, is_new).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Alias saved.".to_string(),
                        is_error: false,
                    });
                    editing_alias.set(None);
                    load_aliases_action(is_loading, aliases, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: err,
                        is_error: true,
                    });
                }
            }
        });
    };

    let on_delete_alias = move |alias: String| {
        spawn(async move {
            match delete_admin_alias(&alias).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "Alias deleted.".to_string(),
                        is_error: false,
                    });
                    load_aliases_action(is_loading, aliases, toasts);
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: err,
                        is_error: true,
                    });
                }
            }
        });
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
                            IconShuffle { size: 20 }
                            span { "Model Aliases & Redirects" }
                        }
                        p { class: "card-description", "Map friendly shortnames (e.g. 'gpt-4o' or 'code-fast') to specific internal or upstream model identifiers." }
                    }
                    button {
                        class: "glass-btn btn-primary",
                        onclick: open_create_modal,
                        IconPlus { size: 16 }
                        span { "Add Alias" }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Alias Name" }
                                th { "Target Model" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if aliases.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 3,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading aliases..." } else { "No model aliases configured." }
                                    }
                                }
                            }
                            for item in aliases.read().iter() {
                                tr {
                                    key: "{item.alias}",
                                    td {
                                        GlassBadge { variant: "glow", "{item.alias}" }
                                    }
                                    td { span { class: "font-mono font-bold text-sm", "{item.target}" } }
                                    td {
                                        class: "text-right",
                                        div {
                                            class: "flex items-center justify-end gap-2",
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let a = item.clone();
                                                    move |_| {
                                                        form_alias.set(a.alias.clone());
                                                        form_target.set(a.target.clone());
                                                        editing_alias.set(Some((a.clone(), false)));
                                                    }
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "glass-btn btn-danger btn-sm",
                                                onclick: {
                                                    let a = item.alias.clone();
                                                    move |_| on_delete_alias(a.clone())
                                                },
                                                "Delete"
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Edit Modal
            if let Some((_, is_new)) = editing_alias.read().clone() {
                GlassModal {
                    title: if is_new { "Create Model Alias".to_string() } else { format!("Edit Alias: {}", form_alias.read()) },
                    on_close: move |_| editing_alias.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Alias Name" }
                            input {
                                class: "glass-input font-mono",
                                disabled: !is_new,
                                placeholder: "e.g. gpt-4o or default",
                                value: "{form_alias.read()}",
                                oninput: move |evt| form_alias.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Target Model Identifier" }
                            input {
                                class: "glass-input font-mono",
                                placeholder: "meta-llama/Llama-3-8B-Instruct",
                                value: "{form_target.read()}",
                                oninput: move |evt| form_target.set(evt.value()),
                            }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| editing_alias.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_alias,
                                "Save Alias"
                            }
                        }
                    }
                }
            }
        }
    }
}
