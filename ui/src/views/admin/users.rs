use crate::api::{
    create_token_for_user, delete_admin_user, fetch_admin_users, save_admin_user,
};
use crate::components::icons::*;
use crate::components::{GlassBadge, GlassCard, GlassModal};
use crate::models::{AdminUserRecord, CreatedTokenResponse};
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::HashSet;
use web_sys::window;

fn load_users_action(
    mut is_loading: Signal<bool>,
    mut users: Signal<Vec<AdminUserRecord>>,
    mut toasts: Signal<Vec<Toast>>,
) {
    is_loading.set(true);
    spawn(async move {
        match fetch_admin_users().await {
            Ok(list) => users.set(list),
            Err(e) => {
                toasts.write().push(Toast {
                    id: js_sys::Date::now() as u64,
                    message: format!("Failed to load users: {e}"),
                    is_error: true,
                });
            }
        }
        is_loading.set(false);
    });
}

#[component]
pub fn AdminUsersView(toasts: Signal<Vec<Toast>>) -> Element {
    let users = use_signal(Vec::<AdminUserRecord>::new);
    let is_loading = use_signal(|| false);
    let mut editing_user = use_signal(|| Option::<(AdminUserRecord, bool)>::None);
    let mut generated_token = use_signal(|| Option::<CreatedTokenResponse>::None);
    let mut copied = use_signal(|| false);

    // Form inputs
    let mut form_username = use_signal(String::new);
    let mut form_is_admin = use_signal(|| false);
    let mut form_log_body = use_signal(|| true);
    let mut form_models = use_signal(String::new);

    let _ = use_resource(move || async move {
        load_users_action(is_loading, users, toasts);
    });

    let open_create_modal = move |_| {
        form_username.set(String::new());
        form_is_admin.set(false);
        form_log_body.set(true);
        form_models.set(String::new());

        editing_user.set(Some((
            AdminUserRecord {
                username: String::new(),
                limit_to_models: None,
                log_request_body: Some(true),
                is_admin: false,
            },
            true,
        )));
    };

    let save_user = move |_| {
        let (_, is_new) = match editing_user.read().clone() {
            Some(u) => u,
            None => return,
        };

        let username = form_username.read().trim().to_string();
        if username.is_empty() {
            return;
        }

        let models = if form_models.read().trim().is_empty() {
            None
        } else {
            Some(
                form_models
                    .read()
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<HashSet<_>>(),
            )
        };

        let payload = AdminUserRecord {
            username,
            limit_to_models: models,
            log_request_body: Some(*form_log_body.read()),
            is_admin: *form_is_admin.read(),
        };

        spawn(async move {
            match save_admin_user(&payload, is_new).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "User saved successfully.".to_string(),
                        is_error: false,
                    });
                    editing_user.set(None);
                    load_users_action(is_loading, users, toasts);
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

    let on_delete_user = move |username: String| {
        spawn(async move {
            match delete_admin_user(&username).await {
                Ok(_) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: "User deleted.".to_string(),
                        is_error: false,
                    });
                    load_users_action(is_loading, users, toasts);
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

    let on_generate_token_for_user = move |username: String| {
        spawn(async move {
            match create_token_for_user(&username).await {
                Ok(res) => {
                    generated_token.set(Some(res));
                }
                Err(err) => {
                    toasts.write().push(Toast {
                        id: js_sys::Date::now() as u64,
                        message: format!("Failed to generate token: {err}"),
                        is_error: true,
                    });
                }
            }
        });
    };

    let copy_gen_token = {
        move |_| {
            if let Some(res) = generated_token.read().as_ref() {
                if let Some(w) = window() {
                    let nav = w.navigator();
                    let clip = nav.clipboard();
                    let _ = clip.write_text(&res.token);
                    copied.set(true);
                }
            }
        }
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
                            IconUsers { size: 20 }
                            span { "User & Role Administration" }
                        }
                        p { class: "card-description", "Manage user authorization, admin status, model access restrictions, and generate user API tokens." }
                    }
                    button {
                        class: "glass-btn btn-primary",
                        onclick: open_create_modal,
                        IconPlus { size: 16 }
                        span { "Add User" }
                    }
                }

                div {
                    class: "glass-table-container",
                    table {
                        class: "glass-table",
                        thead {
                            tr {
                                th { "Username" }
                                th { "Role" }
                                th { "Model Access" }
                                th { "Payload Logging" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if users.read().is_empty() {
                                tr {
                                    td {
                                        colspan: 5,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() { "Loading users..." } else { "No users registered." }
                                    }
                                }
                            }
                            for user in users.read().iter() {
                                tr {
                                    key: "{user.username}",
                                    td { span { class: "font-mono font-bold text-sm", "{user.username}" } }
                                    td {
                                        if user.is_admin {
                                            GlassBadge { variant: "glow", "Administrator" }
                                        } else {
                                            GlassBadge { "Standard User" }
                                        }
                                    }
                                    td {
                                        if let Some(ref m) = user.limit_to_models {
                                            GlassBadge { variant: "warning", "{m.len()} models allowed" }
                                        } else {
                                            GlassBadge { variant: "success", "Unrestricted (All Models)" }
                                        }
                                    }
                                    td {
                                        if user.log_request_body.unwrap_or(true) {
                                            span { class: "text-xs text-muted", "Enabled" }
                                        } else {
                                            span { class: "text-xs text-dim", "Disabled" }
                                        }
                                    }
                                    td {
                                        class: "text-right",
                                        div {
                                            class: "flex items-center justify-end gap-2",
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let un = user.username.clone();
                                                    move |_| on_generate_token_for_user(un.clone())
                                                },
                                                IconKey { size: 14 }
                                                span { "Token" }
                                            }
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let u = user.clone();
                                                    move |_| {
                                                        form_username.set(u.username.clone());
                                                        form_is_admin.set(u.is_admin);
                                                        form_log_body.set(u.log_request_body.unwrap_or(true));
                                                        form_models.set(
                                                            u.limit_to_models
                                                                .as_ref()
                                                                .map(|m| m.iter().cloned().collect::<Vec<_>>().join(", "))
                                                                .unwrap_or_default(),
                                                        );

                                                        editing_user.set(Some((u.clone(), false)));
                                                    }
                                                },
                                                "Edit"
                                            }
                                            button {
                                                class: "glass-btn btn-danger btn-sm",
                                                onclick: {
                                                    let un = user.username.clone();
                                                    move |_| on_delete_user(un.clone())
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

            // Edit User Modal
            if let Some((_, is_new)) = editing_user.read().clone() {
                GlassModal {
                    title: if is_new { "Create User".to_string() } else { format!("Edit User: {}", form_username.read()) },
                    on_close: move |_| editing_user.set(None),
                    div {
                        class: "flex flex-col gap-3",
                        
                        div {
                            class: "form-group",
                            label { class: "form-label", "Username" }
                            input {
                                class: "glass-input font-mono",
                                disabled: !is_new,
                                value: "{form_username.read()}",
                                oninput: move |evt| form_username.set(evt.value()),
                            }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Allowed Models (comma-separated, empty for all)" }
                            input {
                                class: "glass-input font-mono",
                                placeholder: "meta-llama/Llama-3-8B-Instruct, mistralai/Mistral-7B",
                                value: "{form_models.read()}",
                                oninput: move |evt| form_models.set(evt.value()),
                            }
                        }

                        div {
                            class: "flex items-center gap-2 mt-2",
                            input {
                                r#type: "checkbox",
                                id: "user-admin-check",
                                checked: *form_is_admin.read(),
                                onchange: move |evt| form_is_admin.set(evt.checked()),
                            }
                            label { r#for: "user-admin-check", class: "text-sm font-bold", "Grant Administrator Privileges" }
                        }

                        div {
                            class: "flex items-center gap-2",
                            input {
                                r#type: "checkbox",
                                id: "user-log-body-check",
                                checked: *form_log_body.read(),
                                onchange: move |evt| form_log_body.set(evt.checked()),
                            }
                            label { r#for: "user-log-body-check", class: "text-sm", "Log Request / Response Body Payloads" }
                        }

                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| editing_user.set(None),
                                "Cancel"
                            }
                            button {
                                class: "glass-btn btn-primary",
                                onclick: save_user,
                                "Save User"
                            }
                        }
                    }
                }
            }

            // Token Generated on Behalf of User Modal
            if let Some(res) = generated_token.read().clone() {
                GlassModal {
                    title: format!("Token Generated for {}", res.username),
                    on_close: move |_| {
                        generated_token.set(None);
                        copied.set(false);
                    },
                    div {
                        class: "flex flex-col gap-3",
                        p { class: "text-sm text-muted", "{res.message}" }
                        div {
                            class: "flex items-center gap-2",
                            input {
                                class: "glass-input font-mono text-sm",
                                readonly: true,
                                value: "{res.token}",
                            }
                            button {
                                class: "glass-btn btn-primary btn-sm",
                                onclick: copy_gen_token,
                                if *copied.read() {
                                    IconCheck { size: 14 }
                                    span { "Copied" }
                                } else {
                                    span { "Copy" }
                                }
                            }
                        }
                        div {
                            class: "modal-footer",
                            button {
                                class: "glass-btn btn-secondary",
                                onclick: move |_| {
                                    generated_token.set(None);
                                    copied.set(false);
                                },
                                "Done"
                            }
                        }
                    }
                }
            }
        }
    }
}
