use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Avatar {
    #[serde(default)]
    pub id: Uuid,
    #[serde(default)]
    pub data_uri: Arc<str>,
    #[serde(default)]
    pub updated_at: u64,
}

impl Avatar {
    pub fn is_empty(&self) -> bool {
        self.data_uri.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use wasm_bindgen_test::wasm_bindgen_test;

    use super::Avatar;

    #[wasm_bindgen_test]
    fn round_trip_serde() {
        let id = Uuid::new_v4();
        let avatar = Avatar {
            id,
            data_uri: "data:image/webp;base64,AAAA".into(),
            updated_at: 12345,
        };
        let json = serde_json::to_string(&avatar).unwrap();
        let restored: Avatar = serde_json::from_str(&json).unwrap();
        assert_eq!(avatar, restored);
    }

    #[wasm_bindgen_test]
    fn deserialize_with_missing_fields() {
        let restored: Avatar = serde_json::from_str("{}").unwrap();
        assert_eq!(restored, Avatar::default());
        assert!(restored.is_empty());
    }

    #[wasm_bindgen_test]
    fn deserialize_legacy_doc_without_id() {
        // Old docs in Firestore have no `id` field — must deserialize to nil UUID
        let json = r#"{"data_uri":"data:image/webp;base64,AAAA","updated_at":1}"#;
        let avatar: Avatar = serde_json::from_str(json).unwrap();
        assert!(avatar.id.is_nil());
        assert_eq!(avatar.updated_at, 1);
    }
}
