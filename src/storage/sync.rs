use std::{cell::RefCell, collections::HashSet};

use futures::future::join_all;
use gloo_storage::{LocalStorage, Storage};
use leptos::prelude::*;
use reactive_stores::Store;
use uuid::Uuid;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::spawn_local;

use crate::{
    ai::Story,
    firebase::{self, ChangeType, FirebaseError},
    model::Character,
    storage::{local, migrate, queue, queue::CloudOp},
};

/// 2 s debounce — balances responsiveness vs. Firestore write-per-second cost.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncStatus {
    Disabled,
    Connecting,
    Synced,
    Syncing,
    Error,
}

#[derive(Clone, Copy)]
pub(super) struct SyncState {
    status: RwSignal<SyncStatus>,
    uid: RwSignal<Option<String>>,
    anon: RwSignal<bool>,
    last_error: RwSignal<Option<String>>,
    /// Bumped after cloud pull modifies the character index, so the UI can
    /// react.
    pub(super) index_version: RwSignal<u32>,
    /// Set to `true` once the initial cloud sync completes (success or
    /// failure). Before this, auto-save skips `touch()` to preserve
    /// timestamps for accurate conflict resolution.
    sync_done: RwSignal<bool>,
}

impl SyncState {
    pub(super) fn set_error(&self, msg: String) {
        self.last_error.set(Some(msg));
        self.status.set(SyncStatus::Error);
    }

    fn set_ok(&self, status: SyncStatus) {
        self.last_error.set(None);
        self.status.set(status);
    }

    pub(super) fn set_syncing(&self) {
        self.set_ok(SyncStatus::Syncing);
    }

    pub(super) fn set_synced(&self) {
        self.set_ok(SyncStatus::Synced);
    }
}

thread_local! {
    static SYNC_STATE: RefCell<Option<SyncState>> = const { RefCell::new(None) };
    static SNAPSHOT_SUBSCRIPTION: RefCell<Option<firebase::Subscription>> = const { RefCell::new(None) };
}

pub(super) fn get_or_init_sync() -> SyncState {
    SYNC_STATE.with(|cell| {
        let mut opt = cell.borrow_mut();
        *opt.get_or_insert_with(|| SyncState {
            status: RwSignal::new(SyncStatus::Disabled),
            uid: RwSignal::new(None),
            anon: RwSignal::new(false),
            last_error: RwSignal::new(None),
            index_version: RwSignal::new(0),
            sync_done: RwSignal::new(false),
        })
    })
}

fn set_snapshot_subscription(subscription: firebase::Subscription) {
    // Dropping the previous Subscription automatically unsubscribes
    SNAPSHOT_SUBSCRIPTION.with(|cell| {
        *cell.borrow_mut() = Some(subscription);
    });
}

pub fn sync_status() -> ReadSignal<SyncStatus> {
    get_or_init_sync().status.read_only()
}

pub fn sync_is_anonymous() -> ReadSignal<bool> {
    get_or_init_sync().anon.read_only()
}

pub fn sync_last_error() -> ReadSignal<Option<String>> {
    get_or_init_sync().last_error.read_only()
}

/// Reactive signal bumped whenever cloud pull updates the character index.
pub fn sync_index_version() -> ReadSignal<u32> {
    get_or_init_sync().index_version.read_only()
}

/// Reactive signal that becomes `true` once the initial cloud sync completes
/// (success, failure, or disabled). Before this, auto-save preserves existing
/// timestamps so sync can compare them accurately.
fn initial_sync_done() -> ReadSignal<bool> {
    get_or_init_sync().sync_done.read_only()
}

/// Set up auto-save and cloud sync pull for a character store.
pub fn setup_auto_save(store: Store<Character>) {
    let sync_done = initial_sync_done();
    let cloud_updating = RwSignal::new(false);

    Effect::new(move || {
        store.track();
        if cloud_updating.get_untracked() {
            cloud_updating.update_untracked(|v| *v = false);
            return;
        }
        if sync_done.get() {
            store.update_untracked(|character| {
                character.touch();
                local::save_character(character);
                schedule_cloud_push(character);
            });
        } else {
            local::save_character(&store.read_untracked());
        }
    });

    let index_version = sync_index_version();
    Effect::new(move |previous: Option<u32>| {
        let (id, local_at) = {
            let character = store.read_untracked();
            (character.id, character.updated_at)
        };
        if previous.is_some() {
            let index_at = local::load_index()
                .characters
                .get(&id)
                .map(|summary| summary.updated_at)
                .unwrap_or(0);
            if index_at > local_at
                && let Some(character) = local::load_character(&id)
            {
                cloud_updating.update_untracked(|v| *v = true);
                store.set(character);
            }
        }
        index_version.get()
    });
}

/// Touch, save to localStorage, and schedule a debounced cloud push.
/// Used by non-reactive callers (import, copy, create).
pub fn save_and_sync_character(character: &mut Character) {
    character.touch();
    local::save_character(character);
    schedule_cloud_push(character);
}

pub fn delete_character(id: &Uuid) {
    local::delete_character_local_only(id);

    if let Some(uid) = firebase::current_uid() {
        queue::push(CloudOp::DeleteCharacter { uid, char_id: *id });
    }
}

fn schedule_cloud_push(character: &Character) {
    if !firebase::is_available() {
        return;
    }
    let Some(uid) = get_or_init_sync().uid.get_untracked() else {
        return;
    };
    queue::push(CloudOp::PushCharacter {
        uid,
        char_id: character.id,
    });
}

#[derive(Debug, Clone, Copy)]
enum SyncOp {
    PullOnly,
    FullSync,
}

impl SyncOp {
    fn for_anon(is_anon: bool) -> Self {
        if is_anon {
            Self::PullOnly
        } else {
            Self::FullSync
        }
    }
}

pub fn init_sync() {
    let state = get_or_init_sync();
    state.set_ok(SyncStatus::Connecting);
    queue::start_flush_interval(2000);

    spawn_local(async move {
        if !firebase::wait_ready().await {
            log::info!("Firebase not available, cloud sync disabled");
            state.status.set(SyncStatus::Disabled);
            state.sync_done.set(true);
            return;
        }

        if let Some((uid, is_anon)) = firebase::wait_for_auth().await {
            log::info!("Auth settled: uid={uid}, anon={is_anon}");
            finish_sign_in(state, is_anon, SyncOp::for_anon(is_anon)).await;
            state.sync_done.set(true);
            return;
        }

        log::info!("No existing session, signing in anonymously");
        match firebase::sign_in_anonymously().await {
            Ok(_) => finish_sign_in(state, true, SyncOp::PullOnly).await,
            Err(error) => {
                log::warn!("Anonymous sign-in failed: {error:?}");
                state.set_error(format!("Anonymous sign-in failed: {error}"));
            }
        }
        state.sync_done.set(true);
    });
}

pub fn sign_in_with_google() {
    if !firebase::is_available() {
        return;
    }
    let state = get_or_init_sync();
    state.set_ok(SyncStatus::Connecting);

    let promise = match firebase::link_with_google_start() {
        Ok(p) => p,
        Err(error) => {
            log::warn!("Google sign-in failed: {error:?}");
            state.set_error(format!("Google sign-in failed: {error}"));
            return;
        }
    };

    spawn_local(async move {
        match firebase::link_with_google_finish(promise).await {
            Ok(_) => finish_sign_in(state, false, SyncOp::FullSync).await,
            Err(error) => {
                log::warn!("Google sign-in failed: {error:?}");
                state.set_error(format!("Google sign-in failed: {error}"));
            }
        }
    });
}

pub fn retry_sync() {
    let state = get_or_init_sync();
    if state.uid.get_untracked().is_none() {
        init_sync();
        return;
    }
    let is_anon = state.anon.get_untracked();
    state.set_ok(SyncStatus::Syncing);
    spawn_local(async move {
        finish_sign_in(state, is_anon, SyncOp::for_anon(is_anon)).await;
    });
}

async fn finish_sign_in(state: SyncState, is_anon: bool, sync_op: SyncOp) {
    state.anon.set(is_anon);
    match firebase::current_uid() {
        Some(uid) => {
            log::info!("finish_sign_in: uid={uid}, op={sync_op:?}");
            state.uid.set(Some(uid.clone()));
            state.set_ok(SyncStatus::Syncing);
            let push_local_only = matches!(sync_op, SyncOp::FullSync);
            let last_err = match sync_all_with_cloud(push_local_only).await {
                Ok(()) => None,
                Err(error) => {
                    log::warn!("Cloud sync failed: {error:?}");
                    Some(format!("Sync failed: {error}"))
                }
            };
            log::info!("finish_sign_in: done");

            // Subscribe to realtime changes after initial sync
            subscribe_to_changes(&uid);

            match last_err {
                Some(msg) => state.set_error(msg),
                None => state.set_ok(SyncStatus::Synced),
            }
        }
        None => {
            state.uid.set(None);
            state.set_error("No UID after sign-in".into());
        }
    }
}

fn subscribe_to_changes(uid: &str) {
    let last_sync = local::load_index()
        .characters
        .values()
        .map(|summary| summary.updated_at)
        .max()
        .unwrap_or(0);

    match firebase::subscribe_collection(
        &["users", uid, "characters"],
        &[firebase::WhereClause(
            "updated_at",
            ">",
            JsValue::from_f64(last_sync as f64),
        )],
        move |changes| {
            let mut index = local::load_index();
            let mut index_dirty = false;

            for change in changes {
                match change.change_type {
                    ChangeType::Added | ChangeType::Modified => {
                        let Some(character) = migrate::deserialize_character_value(change.data)
                        else {
                            log::warn!("Failed to deserialize snapshot character");
                            continue;
                        };
                        let local_at = index
                            .characters
                            .get(&character.id)
                            .map(|summary| summary.updated_at)
                            .unwrap_or(0);
                        if character.updated_at > local_at {
                            if let Err(error) =
                                LocalStorage::set(local::character_key(&character.id), &character)
                            {
                                log::warn!("Failed to save pulled character: {error}");
                                continue;
                            }
                            index.characters.insert(character.id, character.summary());
                            index_dirty = true;
                        }
                    }
                    ChangeType::Removed => {
                        if let Ok(id) = change.id.parse::<Uuid>() {
                            LocalStorage::delete(local::character_key(&id));
                            index.characters.shift_remove(&id);
                            index_dirty = true;
                        }
                    }
                }
            }

            if index_dirty {
                local::save_index(&index);
                get_or_init_sync()
                    .index_version
                    .update(|version| *version += 1);
            }
        },
    ) {
        Ok(subscription) => set_snapshot_subscription(subscription),
        Err(error) => log::warn!("Failed to subscribe to character changes: {error}"),
    }
}

async fn push_to_cloud(uid: &str, character: &Character) -> Result<(), FirebaseError> {
    firebase::set_doc(
        character,
        &["users", uid, "characters", &character.id.to_string()],
    )
    .await
}

async fn sync_all_with_cloud(push_local_only: bool) -> Result<(), FirebaseError> {
    let Some(uid) = firebase::current_uid() else {
        log::info!("sync_all_with_cloud: no UID, skipping");
        return Ok(());
    };
    log::info!("sync_all_with_cloud: syncing for uid={uid}");
    let remote_chars =
        firebase::get_all_docs::<serde_json::Value>(&["users", &uid, "characters"]).await?;
    log::info!(
        "sync_all_with_cloud: got {} remote characters",
        remote_chars.len()
    );

    let mut index = local::load_index();
    let mut index_dirty = false;
    let mut push_failures: u32 = 0;
    let mut seen_remote: HashSet<Uuid> = HashSet::with_capacity(remote_chars.len());

    for remote_value in remote_chars {
        let remote: Character = match migrate::deserialize_character_value(remote_value) {
            Some(character) => character,
            None => {
                log::warn!("Failed to deserialize remote character (migration failed)");
                continue;
            }
        };
        seen_remote.insert(remote.id);

        let local_updated_at = index
            .characters
            .get(&remote.id)
            .map(|summary| summary.updated_at)
            .unwrap_or(0);

        if local_updated_at >= remote.updated_at {
            if local_updated_at > remote.updated_at
                && let Some(local_character) = local::load_character(&remote.id)
                && let Err(error) = push_to_cloud(&uid, &local_character).await
            {
                log::warn!("Failed to push local-newer character: {error:?}");
                push_failures += 1;
            }
        } else {
            if let Err(error) = LocalStorage::set(local::character_key(&remote.id), &remote) {
                log::warn!("Failed to save pulled character {}: {error}", remote.id);
                continue;
            }

            let summary = remote.summary();
            index.characters.insert(summary.id, summary);
            index_dirty = true;
        }
    }

    if push_local_only {
        for summary in index.characters.values() {
            if !seen_remote.contains(&summary.id)
                && let Some(character) = local::load_character(&summary.id)
            {
                log::info!("sync_all_with_cloud: pushing local-only {}", summary.id);
                if let Err(error) = push_to_cloud(&uid, &character).await {
                    log::warn!("Failed to push local-only character: {error:?}");
                    push_failures += 1;
                }
                // Also push stories for local-only characters
                queue::push(CloudOp::PushStories {
                    uid: uid.clone(),
                    char_id: summary.id,
                });
            }
        }
    }

    if index_dirty {
        local::save_index(&index);
        get_or_init_sync().index_version.update(|v| *v += 1);
    }

    // Sync stories only for characters that were seen remotely (changed on another
    // device)
    let story_results = join_all(
        seen_remote
            .iter()
            .map(|char_id| sync_stories_with_cloud(&uid, char_id)),
    )
    .await;
    for result in story_results {
        if let Err(error) = result {
            log::warn!("Story sync failed: {error:?}");
        }
    }

    if push_failures > 0 {
        Err(FirebaseError::Js(JsValue::from_str(&format!(
            "Failed to push {push_failures} character(s)"
        ))))
    } else {
        Ok(())
    }
}

async fn sync_stories_with_cloud(uid: &str, char_id: &Uuid) -> Result<(), FirebaseError> {
    let char_id_str = char_id.to_string();
    let remote_stories: Vec<Story> =
        firebase::get_all_docs(&["users", uid, "characters", &char_id_str, "stories"]).await?;
    if remote_stories.is_empty() {
        return Ok(());
    }

    let mut local_stories = local::load_stories(char_id);
    let local_ids: HashSet<Uuid> = local_stories.iter().map(|story| story.id).collect();
    let mut dirty = false;

    for remote_story in remote_stories {
        if !local_ids.contains(&remote_story.id) {
            local_stories.push(remote_story);
            dirty = true;
        }
    }

    if dirty {
        local_stories.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        local::save_stories(char_id, &local_stories);
    }

    Ok(())
}
