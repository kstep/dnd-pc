use leptos::prelude::*;

use crate::{expr::Expr, model::Attribute};

/// Renders an expression formula as a styled `<pre><code>` block with wrapping.
#[component]
pub fn ExprView(expr: Expr<Attribute>) -> impl IntoView {
    view! { <pre class="expr-view"><code>{expr.to_string()}</code></pre> }
}
