use leptos::prelude::*;
use pulldown_cmark::{Options, Parser, html};

#[cfg_attr(
    feature = "perf-marks",
    tracing::instrument(name = "markdown.render", skip(md), fields(len = md.len()))
)]
fn render_markdown(md: &str) -> String {
    let parser = Parser::new_ext(md, Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

#[component]
pub fn Markdown(#[prop(into)] text: Signal<String>) -> impl IntoView {
    move || {
        let html = render_markdown(&text.get());
        view! { <div class="markdown" inner_html=html /> }
    }
}
