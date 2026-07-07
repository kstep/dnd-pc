use serde::Deserialize;

use crate::{BASE_URL, rules::utils::fetch_json};

/// Flat name pools; species-specific groups were dropped once the wizard
/// started asking for the name before the species.
#[derive(Clone, Deserialize)]
pub struct NamesData {
    first: Vec<Box<str>>,
    last: Vec<Box<str>>,
}

impl NamesData {
    pub fn generate_name(&self) -> String {
        let first = random_pick(&self.first);
        let last = random_pick(&self.last);
        format!("{first} {last}")
    }
}

fn random_pick(items: &[Box<str>]) -> &str {
    let index = getrandom::u32().unwrap_or(0) as usize % items.len();
    &items[index]
}

pub async fn fetch_names() -> Option<NamesData> {
    fetch_json(&format!("{BASE_URL}/names.json")).await.ok()
}
