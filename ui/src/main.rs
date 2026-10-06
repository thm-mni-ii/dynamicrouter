mod api;
mod app;
mod components;
mod models;
mod state;
mod views;

fn main() {
    dioxus::launch(app::App);
}
