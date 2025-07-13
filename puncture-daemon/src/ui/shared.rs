use maud::{DOCTYPE, Markup, html};

pub fn inline_error(message: &str) -> Markup {
    html! {
        div class="alert alert-danger fade show mt-3 mb-0" {
            (message)
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
                    ".sidebar { background-color: #2c3e50 !important; min-height: 100vh; }"
                    ".sidebar .nav-link { color: #ffffff !important; padding: 0.75rem 1rem; }"
                    ".sidebar .nav-link:hover { background-color: #34495e !important; }"
                    ".sidebar .nav-link.active { }"
                    ".action-sidebar { background-color: #f8f9fa !important; min-height: 100vh; }"
                }
            }
            body {
                div id="toast-container" class="position-fixed top-0 end-0 p-3" style="z-index: 1100;" {}

                nav class="navbar navbar-dark bg-dark" {
                    div class="container-fluid" {
                        span class="navbar-brand" { "Puncture Dashboard" }
                    }
                }
                div class="container-fluid" {
                    div class="row" {
                        nav class="col-md-3 col-lg-2 d-md-block sidebar collapse" {
                            div class="position-sticky pt-3" {
                                ul class="nav flex-column" {
                                    li class="nav-item" {
                                        a class={
                                            "nav-link text-white" @if current_path == "/" { " active" }
                                        } href="/" { "Balances" }
                                    }
                                    li class="nav-item" {
                                        a class={
                                            "nav-link text-white" @if current_path == "/channels" { " active" }
                                        } href="/channels" { "Channels" }
                                    }
                                    li class="nav-item" {
                                        a class={
                                            "nav-link text-white" @if current_path == "/peers" { " active" }
                                        } href="/peers" { "Peers" }
                                    }
                                    li class="nav-item" {
                                        a class={
                                            "nav-link text-white" @if current_path == "/users" { " active" }
                                        } href="/users" { "Users" }
                                    }
                                }
                            }
                        }
                        main class="col-md-6 col-lg-7 px-md-4" {
                            div class="pt-3" {
                                (content)
                            }
                        }
                        aside class="col-md-3 col-lg-3 px-md-4 action-sidebar" {
                            div class="pt-3" {
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
