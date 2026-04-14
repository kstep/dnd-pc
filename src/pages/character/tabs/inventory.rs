use leptos::prelude::*;

use crate::components::panels::equipment::EquipmentPanel;

#[component]
pub fn InventoryTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <EquipmentPanel />
        </div>
    }
}
