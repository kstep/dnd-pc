use dnd_pc::App;
use leptos::prelude::*;

fn main() {
    _ = console_log::init_with_level(log::Level::Debug);
    console_error_panic_hook::set_once();
    #[cfg(feature = "perf-marks")]
    tracing_wasm::set_as_global_default();

    mount_to_body(|| {
        view! {
            <App />
        }
    })
}
