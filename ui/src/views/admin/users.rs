use crate::api::{
    create_token_for_user, delete_admin_user, fetch_admin_users, fetch_v1_models, save_admin_user,
};
use crate::components::icons::*;
use crate::components::{ChipInput, GlassBadge, GlassCard, GlassModal};
use crate::models::{AdminUserRecord, CreatedTokenResponse};
use crate::state::Toast;
use dioxus::prelude::*;
use std::collections::HashSet;
use web_sys::window;

const ONE_WEEK_SECS: i64 = 7 * 24 * 60 * 60;

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
    let mut model_suggestions = use_signal(Vec::<String>::new);
    let is_loading = use_signal(|| false);
    let mut editing_user = use_signal(|| Option::<(AdminUserRecord, bool)>::None);
    let mut generated_token = use_signal(|| Option::<CreatedTokenResponse>::None);
    let mut copied = use_signal(|| false);
    let mut show_all_users = use_signal(|| false);

    // Form inputs
    let mut form_username = use_signal(String::new);
    let mut form_is_admin = use_signal(|| false);
    let mut form_log_body = use_signal(|| true);
    let mut form_models = use_signal(Vec::<String>::new);
    let mut form_expires_at = use_signal(String::new);

    let _ = use_resource(move || async move {
        load_users_action(is_loading, users, toasts);
        if let Ok(models) = fetch_v1_models().await {
            let mut list: Vec<String> = models.into_iter().map(|m| m.id).collect();
            list.sort();
            model_suggestions.set(list);
        }
    });

    let open_create_modal = move |_| {
        form_username.set(String::new());
        form_is_admin.set(false);
        form_log_body.set(true);
        form_models.set(Vec::new());
        form_expires_at.set(String::new());

        editing_user.set(Some((
            AdminUserRecord {
                username: String::new(),
                limit_to_models: None,
                log_request_body: Some(true),
                is_admin: false,
                expires_at: None,
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

        let models = if form_models.read().is_empty() {
            None
        } else {
            Some(form_models.read().iter().cloned().collect::<HashSet<_>>())
        };

        let exp_str = form_expires_at.read().trim().to_string();
        let expires_at = if exp_str.is_empty() {
            None
        } else {
            let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_str(&exp_str));
            let time = d.get_time();
            if time.is_nan() {
                None
            } else {
                Some((time / 1000.0) as i64)
            }
        };

        let payload = AdminUserRecord {
            username,
            limit_to_models: models,
            log_request_body: Some(*form_log_body.read()),
            is_admin: *form_is_admin.read(),
            expires_at,
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
            match create_token_for_user(&username, None).await {
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

    let now = (js_sys::Date::now() / 1000.0) as i64;
    let show_all = *show_all_users.read();
    let users_list = users.read().clone();
    let filtered_users: Vec<AdminUserRecord> = users_list
        .into_iter()
        .filter(|user| {
            if show_all {
                return true;
            }
            if let Some(exp) = user.expires_at {
                exp + ONE_WEEK_SECS >= now
            } else {
                true
            }
        })
        .collect();

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
                    div {
                        class: "flex items-center gap-3",
                        label {
                            class: "flex items-center gap-2 cursor-pointer text-sm text-muted select-none",
                            input {
                                r#type: "checkbox",
                                checked: *show_all_users.read(),
                                onchange: move |evt| show_all_users.set(evt.checked()),
                            }
                            span { "Show all users" }
                        }
                        button {
                            class: "glass-btn btn-primary",
                            onclick: open_create_modal,
                            IconPlus { size: 16 }
                            span { "Add User" }
                        }
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
                                th { "Expires" }
                                th { class: "text-right", "Actions" }
                            }
                        }
                        tbody {
                            if filtered_users.is_empty() {
                                tr {
                                    td {
                                        colspan: 6,
                                        class: "text-muted",
                                        style: "text-align: center; padding: 2rem;",
                                        if *is_loading.read() {
                                            "Loading users..."
                                        } else if users.read().is_empty() {
                                            "No users registered."
                                        } else {
                                            "No active users (expired users older than 1 week are hidden). Check 'Show all users' to view them."
                                        }
                                    }
                                }
                            }
                            for user in filtered_users.iter() {
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
                                        {
                                            if let Some(exp) = user.expires_at {
                                                let now = (js_sys::Date::now() / 1000.0) as i64;
                                                let date_str = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((exp * 1000) as f64))
                                                    .to_locale_string("en-US", &wasm_bindgen::JsValue::UNDEFINED)
                                                    .as_string()
                                                    .unwrap_or_else(|| format!("{exp}"));
                                                if now > exp {
                                                    rsx! { GlassBadge { variant: "danger", "Expired ({date_str})" } }
                                                } else {
                                                    rsx! { GlassBadge { variant: "warning", "{date_str}" } }
                                                }
                                            } else {
                                                rsx! { GlassBadge { "Never" } }
                                            }
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
                                                    let _un = user.username.clone();
                                                    move |_| {
                                                        if let Some(w) = window() {
                                                            let _ = w.location().set_pathname("/admin/rate-limits");
                                                        }
                                                    }
                                                },
                                                IconGauge { size: 14 }
                                                span { "Limits" }
                                            }
                                            button {
                                                class: "glass-btn btn-secondary btn-sm",
                                                onclick: {
                                                    let u = user.clone();
                                                    move |_| {
                                                        form_username.set(u.username.clone());
                                                        form_is_admin.set(u.is_admin);
                                                        form_log_body.set(u.log_request_body.unwrap_or(true));
                                                        let mut m_list: Vec<String> = u.limit_to_models
                                                            .as_ref()
                                                            .map(|m| m.iter().cloned().collect())
                                                            .unwrap_or_default();
                                                        m_list.sort();
                                                        form_models.set(m_list);

                                                        if let Some(exp) = u.expires_at {
                                                            let d = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64((exp * 1000) as f64));
                                                            let iso = d.to_iso_string().as_string().unwrap_or_default();
                                                            if iso.len() >= 16 {
                                                                form_expires_at.set(iso[..16].to_string());
                                                            } else {
                                                                form_expires_at.set(String::new());
                                                            }
                                                        } else {
                                                            form_expires_at.set(String::new());
                                                        }

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
                            label { class: "form-label", "Allowed Models (empty for access to all models)" }
                            ChipInput {
                                values: form_models,
                                suggestions: model_suggestions.read().clone(),
                                placeholder: "Add allowed model...",
                            }
                            span { class: "form-hint", "Select from known models or enter custom model IDs. Comma or Enter to add." }
                        }

                        div {
                            class: "form-group",
                            label { class: "form-label", "Account Expiration (leave empty for never expires)" }
                            input {
                                r#type: "datetime-local",
                                class: "glass-input",
                                value: "{form_expires_at.read()}",
                                oninput: move |evt| form_expires_at.set(evt.value()),
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
