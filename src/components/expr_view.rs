use leptos::prelude::*;
use leptos_fluent::move_tr;

use crate::{expr::Expr, model::Attribute};

/// Renders an expression formula as a styled `<pre><code>` block with wrapping.
#[component]
pub fn ExprView(expr: Expr<Attribute>) -> impl IntoView {
    view! { <pre class="expr-view"><code>{expr.to_string()}</code></pre> }
}

/// Collapsible expression details: "Show expression" toggle with `ExprView`
/// inside.
#[component]
pub fn ExprDetails(expr: Expr<Attribute>) -> impl IntoView {
    view! {
        <details class="effects-calc-expr">
            <summary>{move_tr!("show-expression")}</summary>
            <ExprView expr />
        </details>
    }
}
