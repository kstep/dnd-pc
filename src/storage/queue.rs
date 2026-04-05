use std::cell::RefCell;

use gloo_storage::{LocalStorage, Storage};
use indexmap::IndexMap;
use uuid::Uuid;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::spawn_local;

use crate::{
    firebase::{self, FirebaseError},
    storage::sync::get_or_init_sync,
};

pub enum CloudOp {
    PushCharacter {
        uid: String,
        char_id: Uuid,
    },
    DeleteCharacter {
        uid: String,
        char_id: Uuid,
    },
    PushStories {
        uid: String,
        char_id: Uuid,
    },
    DeleteStory {
        uid: String,
        char_id: Uuid,
        story_id: Uuid,
    },
}

#[derive(Hash, PartialEq, Eq)]
enum QueueKey {
    Character(Uuid),
    Stories(Uuid),
    Story(Uuid, Uuid),
}

impl CloudOp {
    fn queue_key(&self) -> QueueKey {
        match self {
            Self::PushCharacter { char_id, .. } | Self::DeleteCharacter { char_id, .. } => {
                QueueKey::Character(*char_id)
            }
            Self::PushStories { char_id, .. } => QueueKey::Stories(*char_id),
            Self::DeleteStory {
                char_id, story_id, ..
            } => QueueKey::Story(*char_id, *story_id),
        }
    }
}

thread_local! {
    static QUEUE: RefCell<IndexMap<QueueKey, CloudOp>> = RefCell::new(IndexMap::new());
}

pub fn push(op: CloudOp) {
    QUEUE.with(|queue| {
        queue.borrow_mut().insert(op.queue_key(), op);
    });
}

/// Start the flush interval. Call once at app init.
pub fn start_flush_interval(interval_ms: u32) {
    let interval_ms = interval_ms.min(i32::MAX as u32) as i32;
    let callback = wasm_bindgen::closure::Closure::wrap(Box::new(flush) as Box<dyn Fn()>);
    web_sys::window()
        .expect("no window")
        .set_interval_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            interval_ms,
        )
        .expect("setInterval failed");
    callback.forget();
}

fn flush() {
    let ops: Vec<CloudOp> =
        QUEUE.with(|queue| queue.borrow_mut().drain(..).map(|(_, op)| op).collect());
    if ops.is_empty() {
        return;
    }
    spawn_local(async move {
        let state = get_or_init_sync();
        state.set_syncing();
        let mut had_error = false;
        for op in ops {
            if let Err(error) = execute_op(op).await {
                log::warn!("Cloud op failed: {error}");
                had_error = true;
            }
        }
        if had_error {
            state.set_error("Some cloud operations failed".into());
        } else {
            state.set_synced();
        }
    });
}

async fn execute_op(op: CloudOp) -> Result<(), FirebaseError> {
    match op {
        CloudOp::PushCharacter { uid, char_id } => {
            let char_key = super::local::character_key(&char_id);
            let Ok(Some(raw)) = LocalStorage::raw().get_item(&char_key) else {
                return Ok(());
            };
            let json: serde_json::Value = serde_json::from_str(&raw).map_err(|error| {
                FirebaseError::Js(JsValue::from_str(&format!("JSON parse: {error}")))
            })?;
            let char_id_str = char_id.to_string();
            firebase::set_doc(&json, &["users", &uid, "characters", &char_id_str]).await
        }
        CloudOp::DeleteCharacter { uid, char_id } => {
            let char_id_str = char_id.to_string();
            firebase::delete_doc(&["users", &uid, "characters", &char_id_str]).await
        }
        CloudOp::PushStories { uid, char_id } => {
            let story_key = super::local::stories_key(&char_id);
            let Ok(Some(raw)) = LocalStorage::raw().get_item(&story_key) else {
                return Ok(());
            };
            let stories: Vec<serde_json::Value> = serde_json::from_str(&raw).map_err(|error| {
                FirebaseError::Js(JsValue::from_str(&format!("JSON parse: {error}")))
            })?;
            let char_id_str = char_id.to_string();
            for story_value in &stories {
                let Some(story_id) = story_value["id"].as_str() else {
                    continue;
                };
                firebase::set_doc(
                    story_value,
                    &[
                        "users",
                        &uid,
                        "characters",
                        &char_id_str,
                        "stories",
                        story_id,
                    ],
                )
                .await?;
            }
            Ok(())
        }
        CloudOp::DeleteStory {
            uid,
            char_id,
            story_id,
        } => {
            let char_id_str = char_id.to_string();
            let story_id_str = story_id.to_string();
            firebase::delete_doc(&[
                "users",
                &uid,
                "characters",
                &char_id_str,
                "stories",
                &story_id_str,
            ])
            .await
        }
    }
}
