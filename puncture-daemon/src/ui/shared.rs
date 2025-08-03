use std::str::FromStr;

use bitcoin::secp256k1::PublicKey;
use chrono::DateTime;
use lightning::ln::msgs::SocketAddress;
use maud::{DOCTYPE, Markup, PreEscaped, html};
use qrcode::QrCode;

pub fn format_sats(sats: u64) -> String {
    sats.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(|chunk| std::str::from_utf8(chunk).unwrap())
        .collect::<Vec<&str>>()
        .join(",")
}

pub fn format_timestamp(timestamp_ms: i64) -> String {
    DateTime::from_timestamp_millis(timestamp_ms)
        .unwrap()
        .format("%d %B %Y")
        .to_string()
}

pub fn qr_code_with_copy(data: &str) -> Markup {
    let qr_svg = QrCode::new(data)
        .expect("Failed to generate QR code")
        .render::<qrcode::render::svg::Color>()
        .build();

    html! {
        div class="text-center" {
            div class="mb-3" {
                div class="border rounded p-2 bg-white d-inline-block" style="width: 250px; max-width: 100%;" {
                    div style="width: 100%; height: auto; overflow: hidden;" {
                        (PreEscaped(format!(r#"<div style="width: 100%; height: auto;">{}</div>"#, qr_svg.replace("width=", "data-width=").replace("height=", "data-height=").replace("<svg", r#"<svg style="width: 100%; height: auto; display: block;""#))))
                    }
                }
            }
            button type="button" class="btn btn-outline-primary btn-sm"
                   onclick={(format!("navigator.clipboard.writeText('{}').then(() => {{ this.textContent='Copied!'; setTimeout(() => this.textContent='Copy to Clipboard', 2000); }}).catch(() => alert('Copy failed'))", data))} {
                "Copy to Clipboard"
            }
        }
    }
}

pub fn copyable_hex_input(value: &str, size: Option<&str>) -> Markup {
    let input_size = size.unwrap_or("input-group-sm");
    html! {
        div class=(format!("input-group {}", input_size)) {
            input type="text" class="form-control font-monospace" value=(value) readonly style="min-width: 0;" {}
            button class="btn btn-outline-secondary" type="button" onclick="navigator.clipboard.writeText(this.previousElementSibling.value); this.textContent='✓'; setTimeout(() => this.textContent='📋', 1000)" {
                "📋"
            }
        }
    }
}

pub fn base_template(
    title: &str,
    current_path: &str,
    content: Markup,
    action_sidebar: Markup,
) -> Markup {
    html! {
        (DOCTYPE)
        html {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) " - Puncture Dashboard" }
                link href="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/css/bootstrap.min.css" rel="stylesheet";
                style {
                    ".action-sidebar { background-color: #f8f9fa !important; min-height: 100vh; }"
                }
            }
            body {
                div id="toast-container" class="position-fixed top-0 end-0 p-3" style="z-index: 1100;" {}

                nav class="navbar navbar-expand-lg navbar-dark bg-dark" {
                    div class="container-fluid" {

                        ul class="navbar-nav me-auto" {
                            li class="nav-item" {
                                a class={
                                    "nav-link" @if current_path == "/" || current_path == "/lightning" { " active" }
                                } href="/" { "Lightning" }
                            }
                            li class="nav-item" {
                                a class={
                                    "nav-link" @if current_path == "/onchain" { " active" }
                                } href="/onchain" { "Onchain" }
                            }
                            li class="nav-item" {
                                a class={
                                    "nav-link" @if current_path == "/users" { " active" }
                                } href="/users" { "Users" }
                            }
                        }

                    }
                }
                div class="container-fluid" {
                    div class="row" {
                        main class="col-lg-9 px-md-4" {
                            div class="pt-md-4 pb-md-4" {
                                (content)
                            }
                        }
                        aside class="col-lg-3 px-md-4 action-sidebar" {
                            div class="pt-md-4 pb-md-4" {
                                (action_sidebar)
                            }
                        }
                    }
                }
            }
        }
        script src="https://cdn.jsdelivr.net/npm/bootstrap@5.3.0/dist/js/bootstrap.bundle.min.js" {}
        script src="https://unpkg.com/htmx.org@1.9.10" {}
    }
}

// Helper functions for common parsing operations
pub fn parse_node_id(node_id_str: &str) -> Result<PublicKey, String> {
    node_id_str
        .parse::<PublicKey>()
        .map_err(|_| "Invalid node ID format".to_string())
}

pub fn parse_socket_address(address_str: &str) -> Result<SocketAddress, String> {
    SocketAddress::from_str(address_str).map_err(|_| "Invalid socket address format".to_string())
}

// Helper for success responses that replace forms (like QR codes)
pub fn success_replacement(title: &str, message: &str, content: Markup) -> Markup {
    html! {
        h6 class="mb-2" { (title) }
        p class="mb-3 text-muted" { (message) }
        (content)
    }
}

// Helper for simple success messages
pub fn success_message(message: &str) -> Markup {
    html! {
        div class="alert alert-success mb-0" style="word-wrap: break-word; overflow-wrap: break-word;" {
            (message)
        }
    }
}
