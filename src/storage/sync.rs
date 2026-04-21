use std::{cell::RefCell, collections::HashSet};

use futures::future::join_all;
use leptos::prelude::*;
use reactive_stores::Store;
use uuid::Uuid;
use wasm_bindgen_futures::spawn_local;

use crate::{
    BASE_URL,
    ai::Story,
    components::toast::Toast,
    firebase::{self, ChangeType, FirebaseError, FirebaseUid, Where},
    model::{Avatar, Character},
    storage::{baseline, diff, local, migrate, queue},
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
    uid: RwSignal<Option<FirebaseUid>>,
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
    static AVATAR_SUBSCRIPTION: RefCell<Option<firebase::Subscription>> = const { RefCell::new(None) };
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

fn set_avatar_subscription(subscription: firebase::Subscription) {
    AVATAR_SUBSCRIPTION.with(|cell| {
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

/// `true` when the user is anonymous and sync has reached a stable state
/// (not Disabled and not Connecting). Used to decide whether to nudge the
/// user toward signing in for cross-device sync.
pub fn should_prompt_sign_in() -> Memo<bool> {
    let status = sync_status();
    let is_anonymous = sync_is_anonymous();
    Memo::new(move |_| {
        let current = status.get();
        is_anonymous.get() && current != SyncStatus::Disabled && current != SyncStatus::Connecting
    })
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
            cloud_updating.update_untracked(|updating| *updating = false);
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
        let id = store.read_untracked().id;
        if previous.is_some()
            && let Some(character) = local::load_character(&id)
        {
            cloud_updating.update_untracked(|updating| *updating = true);
            store.set(character);
        }
        index_version.get()
    });
}

/// Set up auto-save and cloud sync pull for an avatar signal.
///
/// Mirrors `setup_auto_save` for the character store. Uses a `cloud_updating`
/// flag to suppress the echo loop: when the cloud-pull effect writes to the
/// signal, the auto-save effect detects the flag on its next run and
/// short-circuits without calling `save_avatar` (which would immediately
/// re-push the just-pulled data).
///
/// A timestamp guard prevents the cloud-pull effect from overwriting in-flight
/// user edits: it only updates the signal when the freshly-loaded avatar is
/// strictly newer than the one currently held in the signal.
pub fn setup_avatar_auto_save(char_id: Uuid, avatar: RwSignal<Option<Avatar>>) {
    let cloud_updating = RwSignal::new(false);

    // Auto-save effect. `previous: Option<()>` skips the initial run without
    // cloning the avatar on every change (avoids requiring Avatar: PartialEq).
    Effect::new(move |previous: Option<()>| {
        let current = avatar.get();
        if previous.is_some() {
            if cloud_updating.get_untracked() {
                cloud_updating.update_untracked(|updating| *updating = false);
            } else {
                match &current {
                    Some(av) if !av.is_empty() => local::save_avatar(&char_id, av),
                    _ => local::remove_avatar(&char_id),
                }
            }
        }
    });

    // Cloud-pull effect: reload avatar from localStorage whenever the index
    // version is bumped, but only overwrite the signal when the loaded avatar
    // is strictly newer than the current one (timestamp guard).
    let index_version = sync_index_version();
    Effect::new(move |previous: Option<u32>| {
        let version = index_version.get();
        if previous.is_some() {
            let loaded = local::load_avatar(&char_id);
            let loaded_ts = loaded.as_ref().map(|avatar| avatar.updated_at).unwrap_or(0);
            let current_ts = avatar
                .get_untracked()
                .as_ref()
                .map(|avatar| avatar.updated_at)
                .unwrap_or(0);
            if loaded_ts > current_ts {
                cloud_updating.set(true);
                avatar.set(loaded);
            }
        }
        version
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
    baseline::remove(id);

    if let Some(uid) = firebase::current_uid() {
        queue::push(queue::CloudOp::DeleteCharacter { uid, char_id: *id });
    }
}

/// Run `f` with the current Firebase UID if available and Firebase is
/// reachable. No-op otherwise. Consolidates the guard boilerplate that
/// all `schedule_*` helpers would otherwise repeat.
fn with_uid(f: impl FnOnce(FirebaseUid)) {
    if !firebase::is_available() {
        return;
    }
    if let Some(uid) = get_or_init_sync().uid.get_untracked() {
        f(uid);
    }
}

fn schedule_cloud_push(character: &Character) {
    let char_id = character.id;
    with_uid(|uid| queue::push(queue::CloudOp::PushCharacter { uid, char_id }));
}

pub fn schedule_avatar_push(char_id: Uuid) {
    with_uid(|uid| queue::push(queue::CloudOp::PushAvatar { uid, char_id }));
}

pub fn schedule_avatar_delete(char_id: Uuid) {
    with_uid(|uid| queue::push(queue::CloudOp::DeleteAvatar { uid, char_id }));
}

pub async fn get_avatar_doc(uid: &str, char_id: Uuid) -> Option<Avatar> {
    let char_id_str = char_id.to_string();
    firebase::get_doc::<Avatar>(&["users", uid, "avatars", &char_id_str])
        .await
        .ok()
        .flatten()
        .filter(|avatar| !avatar.is_empty())
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
    if crate::export::is_telegram_mini_app() {
        let origin = window().location().origin().unwrap_or_default();
        let url = format!("{origin}{BASE_URL}/");
        crate::export::copy_to_clipboard(&url);
        Toast::i18n("toast-login-tma").show();
        return;
    }
    let state = get_or_init_sync();
    state.set_ok(SyncStatus::Connecting);

    let promise = match firebase::link_with_google_start() {
        Ok(promise) => promise,
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
            let last_err = match sync_all_with_cloud(sync_op).await {
                Ok(()) => None,
                Err(error) => {
                    log::warn!("Cloud sync failed: {error:?}");
                    Some(format!("Sync failed: {error}"))
                }
            };
            log::info!("finish_sign_in: done");

            // Subscribe to realtime changes after initial sync
            subscribe_to_changes(&uid);
            subscribe_to_avatar_changes(&uid);

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

fn subscribe_sync_collection<PerChange>(
    uid: &FirebaseUid,
    collection: &'static str,
    load_cursor: fn() -> u64,
    save_cursor: fn(u64),
    store_subscription: fn(firebase::Subscription),
    per_change: PerChange,
) where
    PerChange: FnMut(firebase::DocChange, &mut u64) -> bool + 'static,
{
    let last_sync = load_cursor();
    let path = ["users", uid.as_str(), collection];
    let per_change = RefCell::new(per_change);
    match firebase::subscribe_collection(
        &path,
        &[Where::gt("updated_at", last_sync as f64)],
        move |changes| {
            let mut max_updated = load_cursor();
            let mut dirty = false;
            for change in changes {
                if per_change.borrow_mut()(change, &mut max_updated) {
                    dirty = true;
                }
            }
            if dirty {
                save_cursor(max_updated);
                get_or_init_sync()
                    .index_version
                    .update(|version| *version += 1);
            }
        },
    ) {
        Ok(subscription) => store_subscription(subscription),
        Err(error) => log::warn!("Failed to subscribe to {collection}: {error}"),
    }
}

fn subscribe_to_changes(uid: &FirebaseUid) {
    subscribe_sync_collection(
        uid,
        "characters",
        local::load_last_sync,
        local::save_last_sync,
        set_snapshot_subscription,
        |change, max_updated| match change.change_type {
            ChangeType::Added | ChangeType::Modified => {
                // Migrate remote to current schema, preserving unknown fields.
                let remote_value = migrate::migrate_value(change.data);
                let Some(id) = remote_value
                    .get("id")
                    .and_then(|value| value.as_str())
                    .and_then(|string| string.parse::<Uuid>().ok())
                else {
                    log::warn!("Snapshot character missing/invalid id");
                    return false;
                };
                let remote_ts = remote_value
                    .get("updated_at")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(0);

                // Load local as Value (via migration-aware load_character_value).
                let local_value = local::load_character_value(&id)
                    .unwrap_or_else(|| serde_json::Value::Object(Default::default()));

                // Fallback to local ⇒ every field appears clean (local == baseline) ⇒
                // merge_3way blind-adopts remote on first snapshot after fresh sign-in.
                // Distinct from the push path's empty-object fallback in queue.rs; each
                // direction uses the fallback appropriate for its operation.
                let merged = baseline::with(&id, |baseline| {
                    diff::merge_3way(
                        &local_value,
                        baseline.unwrap_or(&local_value),
                        &remote_value,
                    )
                });

                if !local::save_character_value(&id, &merged) {
                    return false;
                }
                baseline::insert(id, remote_value);
                *max_updated = (*max_updated).max(remote_ts);
                true
            }
            ChangeType::Removed => {
                if let Ok(id) = change.id.parse::<Uuid>() {
                    local::delete_character_local_only(&id);
                    baseline::remove(&id);
                    return true;
                }
                false
            }
        },
    );
}

fn subscribe_to_avatar_changes(uid: &FirebaseUid) {
    subscribe_sync_collection(
        uid,
        "avatars",
        local::load_last_sync_avatars,
        local::save_last_sync_avatars,
        set_avatar_subscription,
        |change, max_seen| match change.change_type {
            ChangeType::Added | ChangeType::Modified => {
                let avatar: Avatar = match serde_json::from_value(change.data) {
                    Ok(avatar) => avatar,
                    Err(error) => {
                        log::warn!("Failed to deserialize snapshot avatar: {error}");
                        return false;
                    }
                };
                let Ok(id) = change.id.parse::<Uuid>() else {
                    log::warn!("avatar doc with unparseable id: {}", change.id);
                    return false;
                };
                let local_at = local::load_avatar_timestamp(&id).unwrap_or(0);
                if avatar.updated_at > local_at {
                    local::save_avatar_quiet(&id, &avatar);
                    *max_seen = (*max_seen).max(avatar.updated_at);
                    return true;
                }
                false
            }
            ChangeType::Removed => {
                if let Ok(char_id) = change.id.parse::<Uuid>() {
                    local::remove_avatar_quiet(&char_id);
                    return true;
                }
                false
            }
        },
    );
}

// TODO: this function is ~200 lines and runs multiple distinct phases
// (character fetch/compare loop, push local-only, stories sync, avatar
// pull, avatar push scheduling). Split into per-phase functions when the
// next phase is added or when we invest in unit-testing the sync layer.
async fn sync_all_with_cloud(sync_op: SyncOp) -> Result<(), FirebaseError> {
    let Some(uid) = firebase::current_uid() else {
        log::info!("sync_all_with_cloud: no UID, skipping");
        return Ok(());
    };
    log::info!("sync_all_with_cloud: syncing for uid={uid}");
    let last_sync = local::load_last_sync();
    let last_avatar_sync = local::load_last_sync_avatars();

    // Hoist once — reused by push-local-only phase and avatar push phase.
    let all_summaries = local::load_all_summaries();

    let remote_chars = firebase::get_all_docs::<serde_json::Value>(
        &["users", uid.as_str(), "characters"],
        &[Where::gt("updated_at", last_sync as f64)],
    )
    .await?;
    log::info!(
        "sync_all_with_cloud: got {} remote characters",
        remote_chars.len()
    );

    let mut max_updated = last_sync;
    let mut dirty = false;
    let mut push_failures: u32 = 0;
    let mut last_push_error: Option<FirebaseError> = None;
    let mut seen_remote: HashSet<Uuid> = HashSet::with_capacity(remote_chars.len());

    for raw_value in remote_chars {
        let migrated = migrate::migrate_value(raw_value);
        let Some(id) = migrated
            .get("id")
            .and_then(|value| value.as_str())
            .and_then(|string| string.parse::<Uuid>().ok())
        else {
            log::warn!("Remote character missing/invalid id");
            continue;
        };
        let remote_ts = migrated
            .get("updated_at")
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        seen_remote.insert(id);

        let local_value = local::load_character_value(&id);
        let local_updated_at = local_value
            .as_ref()
            .and_then(|value| value.get("updated_at"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0);

        if local_updated_at > remote_ts {
            // Local has unpushed edits; push them as a diff against remote.
            let Some(local_value) = local_value else {
                continue;
            };
            let char_path = ["users", uid.as_str(), "characters", &id.to_string()];
            if let Some(diff) = diff::sparse_diff(&local_value, &migrated) {
                match firebase::merge_doc(&diff, &char_path).await {
                    Ok(()) => baseline::insert(id, local_value),
                    Err(error) => {
                        log::warn!("Failed to push local-newer character: {error:?}");
                        push_failures += 1;
                        last_push_error = Some(error);
                    }
                }
            } else {
                // No actual diff despite different timestamps. Seed baseline from the
                // server-canonical form so future diffs stay consistent with the shape
                // of incoming snapshots.
                baseline::insert(id, migrated);
            }
        } else if local_updated_at < remote_ts {
            // Remote is newer; blind save and seed baseline from remote.
            if !local::save_character_value(&id, &migrated) {
                continue;
            }
            baseline::insert(id, migrated);
            max_updated = max_updated.max(remote_ts);
            dirty = true;
        } else {
            // Timestamps equal → synchronized. Seed baseline. We deliberately
            // do NOT advance max_updated here: the invariant "cursor only
            // advances when we pulled something" is simpler, and the redundant
            // re-fetch on next sign-in (when last_sync stays below remote_ts)
            // is bounded by the per-character count.
            baseline::insert(id, migrated);
        }
    }

    if matches!(sync_op, SyncOp::FullSync) {
        for summary in &all_summaries {
            if !seen_remote.contains(&summary.id)
                && let Some(character) = local::load_character(&summary.id)
            {
                log::info!("sync_all_with_cloud: pushing local-only {}", summary.id);
                let char_value = match serde_json::to_value(&character) {
                    Ok(value) => value,
                    Err(error) => {
                        log::warn!("Failed to serialize local-only character: {error}");
                        continue;
                    }
                };
                let path = ["users", uid.as_str(), "characters", &summary.id.to_string()];
                match firebase::set_doc(&char_value, &path).await {
                    Ok(()) => baseline::insert(summary.id, char_value),
                    Err(error) => {
                        log::warn!("Failed to push local-only character: {error:?}");
                        push_failures += 1;
                        last_push_error = Some(error);
                    }
                }
                // Also push stories for local-only characters
                queue::push(queue::CloudOp::PushStories {
                    uid: uid.clone(),
                    char_id: summary.id,
                });
            }
        }
    }

    // Sync stories only for characters that were seen remotely (changed on another
    // device)
    let story_results = join_all(
        seen_remote
            .iter()
            .map(|char_id| sync_stories_with_cloud(uid.as_str(), char_id)),
    )
    .await;
    for result in story_results {
        if let Err(error) = result {
            log::warn!("Story sync failed: {error:?}");
        }
    }

    // --- Avatar pull phase: single batch query instead of N sequential gets ---
    let remote_avatars: Vec<Avatar> = firebase::get_all_docs(
        &["users", uid.as_str(), "avatars"],
        &[Where::gt("updated_at", last_avatar_sync as f64)],
    )
    .await
    .unwrap_or_default();
    log::info!(
        "sync_all_with_cloud: got {} remote avatars",
        remote_avatars.len()
    );

    let mut max_avatar_updated = last_avatar_sync;
    let mut dirty_avatars = false;

    for remote in remote_avatars {
        if remote.id.is_nil() {
            // Legacy doc written before the `id` field was added — can't
            // identify which character it belongs to. Skip; it will be
            // backfilled on the next local push.
            log::warn!("avatar doc with nil id, skipping (legacy)");
            continue;
        }
        let local_ts = local::load_avatar_timestamp(&remote.id).unwrap_or(0);
        if remote.updated_at > local_ts {
            if remote.is_empty() {
                local::remove_avatar_quiet(&remote.id);
            } else {
                local::save_avatar_quiet(&remote.id, &remote);
            }
            max_avatar_updated = max_avatar_updated.max(remote.updated_at);
            dirty_avatars = true;
        }
    }

    // Push: schedule upload for any local avatar newer than last avatar sync.
    for summary in &all_summaries {
        if let Some(local_ts) = summary.avatar_updated_at
            && local_ts > last_avatar_sync
        {
            schedule_avatar_push(summary.id);
        }
    }

    if dirty {
        local::save_last_sync(max_updated);
    }
    if dirty_avatars {
        local::save_last_sync_avatars(max_avatar_updated);
    }
    if dirty || dirty_avatars {
        get_or_init_sync()
            .index_version
            .update(|version| *version += 1);
    }

    if let Some(last_error) = last_push_error {
        Err(FirebaseError::BatchFailed {
            count: push_failures,
            last: Box::new(last_error),
        })
    } else {
        Ok(())
    }
}

async fn sync_stories_with_cloud(uid: &str, char_id: &Uuid) -> Result<(), FirebaseError> {
    let char_id_str = char_id.to_string();
    let remote_stories: Vec<Story> =
        firebase::get_all_docs(&["users", uid, "characters", &char_id_str, "stories"], &[]).await?;
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
