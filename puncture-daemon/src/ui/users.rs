use axum::{
    Form,
    extract::State,
    response::{Html, IntoResponse},
};
use maud::{Markup, html};
use puncture_core::invite::Invite;
use rand::Rng;
use serde::Deserialize;

use super::shared::{base_template, copyable_hex_input};
use crate::AppState;

pub fn users_template(users: &[puncture_cli_core::UserInfo]) -> Markup {
    let content = html! {
        div {
            div class="table-responsive" {
                    table class="table table-hover" {
                        thead {
                            tr {
                                th { "User Public Key" }
                                th { "Balance (msat)" }
                                th { "Created At" }
                            }
                        }
                        tbody {
                            @for user in users {
                                tr {
                                    td {
                                        (copyable_hex_input(&user.user_pk, Some("input-group-sm")))
                                    }
                                    td {
                                        (user.balance_msat.to_string())
                                    }
                                    td {
                                        (user.created_at)
                                    }
                                }
                            }
                        }
                    }
                }
            }
    };

    let action_sidebar = html! {
        div class="card" {
            div class="card-header" {
                h5 class="card-title mb-0" { "Invite Users" }
            }
            div class="card-body" {
                form hx-post="/users/invite"
                     hx-target="#invite-results"
                     hx-swap="innerHTML" {
                    div class="mb-3" {
                        label for="expiry_days" class="form-label" { "Expiry (days)" }
                        input type="number" class="form-control" id="expiry_days" name="expiry_days" value="1" min="1" max="365" required {}
                    }
                    div class="mb-3" {
                        label for="user_limit" class="form-label" { "User Limit" }
                        input type="number" class="form-control" id="user_limit" name="user_limit" value="10" min="1" max="1000" required {}
                    }
                    button type="submit" class="btn btn-outline-primary w-100" { "Generate Invite" }
                }

                div id="invite-results" {}
            }
        }
    };

    base_template("Users", "/users", content, action_sidebar)
}

#[derive(Deserialize)]
pub struct InviteForm {
    #[serde(default = "default_expiry_days")]
    pub expiry_days: u32,
    #[serde(default = "default_user_limit")]
    pub user_limit: u32,
}

fn default_expiry_days() -> u32 {
    1
}

fn default_user_limit() -> u32 {
    10
}

pub async fn users_page(State(state): State<AppState>) -> impl IntoResponse {
    Html(users_template(&super::db::list_users(&state.db).await).into_string())
}

pub async fn invite_submit(
    State(state): State<AppState>,
    Form(form): Form<InviteForm>,
) -> impl IntoResponse {
    let invite_id = rand::rng().random();

    super::db::create_invite(
        &state.db,
        &invite_id,
        form.user_limit,
        form.expiry_days * 24 * 60 * 60,
    )
    .await;

    let invite = Invite::new(invite_id, state.node_id).encode();

    let html = html! {
        div class="alert alert-success fade show mt-3 mb-0" {
            h6 class="mb-2" { "Invite Generated" }
            p class="mb-2" { "Share this invite code with your users:" }
            (copyable_hex_input(&invite, None))
        }
    };

    Html(html.into_string())
}
