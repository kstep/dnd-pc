use std::collections::BTreeMap;

use serde::Deserialize;

pub fn get_for_level<T: Clone + Default>(levels: &BTreeMap<u32, T>, level: u32) -> T {
    levels
        .range(..=level)
        .next_back()
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

pub async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|error| format!("fetch error: {error}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json()
        .await
        .map_err(|error| format!("parse error: {error}"))
}
