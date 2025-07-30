use axum::{Form, extract::State, response::Html};
use maud::{Markup, html};
use puncture_core::PunctureCode;
use rand::Rng;
use serde::Deserialize;

use super::shared::{
    base_template, copyable_hex_input, format_sats, format_timestamp, qr_code_with_copy,
    success_replacement,
};
use crate::AppState;

pub async fn users_page(State(state): State<AppState>) -> Html<String> {
    let users = super::db::list_users(&state.db).await;

    let total_user_count = users.len();

    // Filter to only users with recovery names and sort by recovery_name
    let mut filtered_users: Vec<_> = users
        .into_iter()
        .filter(|user| user.recovery_name.is_some())
        .collect();

    filtered_users.sort_by_key(|a| a.recovery_name.as_ref().unwrap().to_string());

    let html = users_template(&filtered_users, total_user_count);

    Html(html.into_string())
}

fn users_template(users: &[puncture_cli_core::UserInfo], total_user_count: usize) -> Markup {
    let content = html! {
        // Users Accordion
        @if users.is_empty() {
            div class="p-4 text-center text-muted" {
                "No users with recovery names yet."
            }
        } @else {
            div class="accordion" id="usersAccordion" {
                @for (i, user) in users.iter().enumerate() {
                    div class="accordion-item" {
                        h2 class="accordion-header" {
                            button class="accordion-button collapsed" type="button" data-bs-toggle="collapse"
                                    data-bs-target={(format!("#user-{}", i))} aria-expanded="false"
                                    aria-controls={(format!("user-{}", i))} {
                                (user.recovery_name.as_ref().unwrap())
                            }
                        }
                        div id={(format!("user-{}", i))} class="accordion-collapse collapse" data-bs-parent="#usersAccordion" {
                            div class="accordion-body" {
                                table class="table table-sm table-borderless mb-0" {
                                    tbody {
                                        tr {
                                            td class="fw-bold" style="width: 1px; white-space: nowrap;" { "Public Key" }
                                            td style="width: 100%; min-width: 0;" {
                                                (copyable_hex_input(&user.user_pk, None))
                                            }
                                        }
                                        tr {
                                            td class="fw-bold" { "Balance" }
                                            td class="font-monospace" { (format_sats(user.balance_msat / 1000)) " ₿" }
                                        }
                                        tr {
                                            td class="fw-bold" { "Created" }
                                            td class="text-muted font-monospace" { (format_timestamp(user.created_at)) }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let action_sidebar = html! {
        div class="accordion" id="usersActionsAccordion" {
            // Invite Users
            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#inviteUserCollapse" aria-expanded="false" aria-controls="inviteUserCollapse" {
                        "Invite"
                    }
                }
                div id="inviteUserCollapse" class="accordion-collapse collapse" data-bs-parent="#usersActionsAccordion" {
                    div class="accordion-body" {
                        (invite_form())
                    }
                }
            }

            // User Recovery
            div class="accordion-item" {
                h2 class="accordion-header" {
                    button class="accordion-button collapsed" type="button" data-bs-toggle="collapse" data-bs-target="#userRecoveryCollapse" aria-expanded="false" aria-controls="userRecoveryCollapse" {
                        "Recover"
                    }
                }
                div id="userRecoveryCollapse" class="accordion-collapse collapse" data-bs-parent="#usersActionsAccordion" {
                    div class="accordion-body" {
                        (recovery_form())
                    }
                }
            }
        }
    };

    base_template("Users", "/users", content, action_sidebar, total_user_count)
}

// Form structs
#[derive(Deserialize)]
pub struct InviteForm {
    pub expiry_days: u32,
    pub user_limit: u32,
}

#[derive(Deserialize)]
pub struct RecoveryForm {
    pub user_pk: String,
}

// Form components
fn invite_form() -> Markup {
    html! {
        form hx-post="/users/invite"
             hx-target="this"
             hx-swap="outerHTML" {

            div class="mb-3" {
                label for="expiry-days" class="form-label" { "Expiry (days)" }
                input type="number" class="form-control" id="expiry-days" name="expiry_days" value="1" min="1" max="365" required {}
            }
            div class="mb-3" {
                label for="user-limit" class="form-label" { "User Limit" }
                input type="number" class="form-control" id="user-limit" name="user_limit" value="10" min="1" max="1000" required {}
            }
            button type="submit" class="btn btn-outline-primary w-100" { "Generate Invite Code" }
        }
    }
}

fn recovery_form() -> Markup {
    html! {
        form hx-post="/users/recover"
             hx-target="this"
             hx-swap="outerHTML" {

            div class="mb-3" {
                label for="user-pk" class="form-label" { "Public Key" }
                input type="text" class="form-control font-monospace" id="user-pk" name="user_pk" placeholder="852e98f0..." required {}
            }
            button type="submit" class="btn btn-outline-primary w-100" { "Generate Recovery Code" }
        }
    }
}

// Route handlers
pub async fn invite_submit(
    State(state): State<AppState>,
    Form(form): Form<InviteForm>,
) -> Html<String> {
    let invite_id = rand::rng().random();

    super::db::create_invite(
        &state.db,
        &invite_id,
        form.user_limit,
        form.expiry_days * 24 * 60 * 60,
    )
    .await;

    let invite = PunctureCode::invite(invite_id, state.node_id).encode();

    let html = success_replacement(
        "Invite Code Generated",
        "Share this code with your users:",
        qr_code_with_copy(&invite),
    );

    Html(html.into_string())
}

pub async fn recovery_submit(
    State(state): State<AppState>,
    Form(form): Form<RecoveryForm>,
) -> Html<String> {
    // Validate user exists
    if !super::db::user_exists(&state.db, form.user_pk.clone()).await {
        let html = html! {
            div class="alert alert-danger" { "User not found with that public key" }
            (recovery_form())
        };
        return Html(html.into_string());
    }

    // Create recovery code (7 days expiry)
    let recovery_id = rand::rng().random();

    super::db::create_recovery(
        &state.db,
        &recovery_id,
        &form.user_pk,
        7 * 24 * 60 * 60, // 7 days
    )
    .await;

    let recovery = PunctureCode::recovery(recovery_id).encode();

    let html = success_replacement(
        "Recovery Code Generated",
        "Share this code with the user to recover their account:",
        qr_code_with_copy(&recovery),
    );

    Html(html.into_string())
}
