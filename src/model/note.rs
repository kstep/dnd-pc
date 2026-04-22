use reactive_stores::Store;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default, Store)]
pub struct Note {
    #[serde(default)]
    pub created_at: u64,
    #[serde(default)]
    pub level: u32,
    #[serde(default)]
    pub text: String,
}
