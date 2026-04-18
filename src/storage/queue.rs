use std::cell::RefCell;

use gloo_storage::{LocalStorage, Storage};
use indexmap::IndexMap;
use uuid::Uuid;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::spawn_local;

use crate::{
    firebase::{self, FirebaseError, FirebaseUid},
    storage::{baseline, diff, local, sync::get_or_init_sync},
};

pub enum CloudOp {
    PushCharacter {
        uid: FirebaseUid,
        char_id: Uuid,
    },
    DeleteCharacter {
        uid: FirebaseUid,
        char_id: Uuid,
    },
    PushStories {
        uid: FirebaseUid,
        char_id: Uuid,
    },
    DeleteStory {
        uid: FirebaseUid,
        char_id: Uuid,
        story_id: Uuid,
    },
    PushAvatar {
        uid: FirebaseUid,
        char_id: Uuid,
    },
    DeleteAvatar {
        uid: FirebaseUid,
        char_id: Uuid,
    },
}

#[derive(Hash, PartialEq, Eq)]
enum QueueKey {
    Character(Uuid),
    Stories(Uuid),
    Story(Uuid, Uuid),
    Avatar(Uuid),
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
            Self::PushAvatar { char_id, .. } | Self::DeleteAvatar { char_id, .. } => {
                QueueKey::Avatar(*char_id)
            }
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
    // FIXME: Closure::forget leaks the callback permanently. Acceptable
    // because setInterval lives for the tab's lifetime and this is called
    // once from init_sync, but if we ever add clearInterval or re-login
    // flows, store the Closure in a thread_local instead to make the leak
    // explicit and the function idempotent.
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
        let mut failed_count: u32 = 0;
        let mut last_error: Option<FirebaseError> = None;
        for op in ops {
            if let Err(error) = execute_op(op).await {
                log::warn!("Cloud op failed: {error}");
                failed_count += 1;
                last_error = Some(error);
            }
        }
        if let Some(last) = last_error {
            state.set_error(
                FirebaseError::BatchFailed {
                    count: failed_count,
                    last: Box::new(last),
                }
                .to_string(),
            );
        } else {
            state.set_synced();
        }
    });
}

async fn execute_op(op: CloudOp) -> Result<(), FirebaseError> {
    match op {
        CloudOp::PushCharacter { uid, char_id } => {
            // Go through load_character so any legacy-schema blob in
            // localStorage is migrated before diffing against the baseline
            // (which is always post-migration).
            //
            // This path does three passes over the Character tree per flush:
            // deserialize (inside load_character), serialize to Value, then
            // sparse_diff. Acceptable because (a) queue coalescing via
            // IndexMap<QueueKey, _> limits this to at most one run per 2s
            // debounce per character, and (b) for a typical character the
            // total pre-network work is a few hundred microseconds in wasm.
            let Some(current) = local::load_character_value(&char_id) else {
                return Ok(());
            };
            // Fallback to empty object ⇒ sparse_diff yields whole current ⇒ first
            // push after fresh sign-in pushes the full document. Distinct from the
            // pull path's fallback in subscribe_to_changes (which uses local as
            // baseline so merge_3way blind-adopts remote) — the choice is
            // context-dependent and both are correct for their direction.
            let empty = serde_json::Value::Object(Default::default());
            let Some(diff) = baseline::with(&char_id, |baseline| {
                diff::sparse_diff(&current, baseline.unwrap_or(&empty))
            }) else {
                return Ok(());
            };
            let char_id_str = char_id.to_string();
            firebase::merge_doc(&diff, &["users", uid.as_str(), "characters", &char_id_str])
                .await?;
            baseline::insert(char_id, current);
            Ok(())
        }
        CloudOp::DeleteCharacter { uid, char_id } => {
            let char_id_str = char_id.to_string();
            firebase::delete_doc(&["users", uid.as_str(), "characters", &char_id_str]).await
        }
        CloudOp::PushStories { uid, char_id } => {
            let story_key = local::stories_key(&char_id);
            let Ok(Some(raw)) = LocalStorage::raw().get_item(&story_key) else {
                return Ok(());
            };
            let stories: Vec<serde_json::Value> = serde_json::from_str(&raw)?;
            let char_id_str = char_id.to_string();
            for story_value in &stories {
                let Some(story_id) = story_value["id"].as_str() else {
                    continue;
                };
                firebase::set_doc(
                    story_value,
                    &[
                        "users",
                        uid.as_str(),
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
                uid.as_str(),
                "characters",
                &char_id_str,
                "stories",
                &story_id_str,
            ])
            .await
        }
        CloudOp::PushAvatar { uid, char_id } => {
            let key = local::avatar_key(&char_id);
            let Ok(Some(raw)) = LocalStorage::raw().get_item(&key) else {
                return Ok(());
            };
            let json: serde_json::Value = serde_json::from_str(&raw)?;
            let char_id_str = char_id.to_string();
            firebase::set_doc(&json, &["users", uid.as_str(), "avatars", &char_id_str]).await
        }
        CloudOp::DeleteAvatar { uid, char_id } => {
            let char_id_str = char_id.to_string();
            firebase::delete_doc(&["users", uid.as_str(), "avatars", &char_id_str]).await
        }
    }
}
