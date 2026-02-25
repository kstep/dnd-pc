use dnd_pc::App;
use leptos::prelude::*;

fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();

    // Register service worker
    if let Some(window) = web_sys::window() {
        let sw_path = format!("{}/sw.js", dnd_pc::BASE_URL);
        let _ = window.navigator().service_worker().register(&sw_path);
        log::info!("Service worker registration initiated");
    }

    mount_to_body(|| {
        view! {
            <App />
        }
    })
}
