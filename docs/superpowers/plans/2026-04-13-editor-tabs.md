# Character Editor Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Разбить Character Editor на 5 вкладок (Stats / Features / Magic / Inventory / Backstory) с URL-роутингом `/c/:id/{tab}`.

**Architecture:** `CharacterEditor` становится вложенным `ParentRoute` с `CharacterHeader + TabNav + <Outlet/>`. Каждый таб — тонкий компонент-композиция существующих панелей. Переиспользуем готовые панели без изменений.

**Tech Stack:** Leptos 0.8 CSR, `leptos_router` nested routing, `leptos_fluent` для i18n.

---

## File Structure

**Создать:**
- `src/components/tab_nav.rs` — универсальный `TabNav` компонент с `TabItem` структурой.
- `src/pages/character/tabs/mod.rs` — экспорт 5 tab-компонентов.
- `src/pages/character/tabs/stats.rs` — `StatsTab`.
- `src/pages/character/tabs/features.rs` — `FeaturesTab`.
- `src/pages/character/tabs/magic.rs` — `MagicTab`.
- `src/pages/character/tabs/inventory.rs` — `InventoryTab`.
- `src/pages/character/tabs/backstory.rs` — `BackstoryTab`.

**Модифицировать:**
- `src/components/mod.rs` — добавить `pub mod tab_nav`.
- `src/pages/character/mod.rs` — добавить `pub mod tabs`.
- `src/pages/character/editor.rs` — переделать из страницы в layout с tab nav и outlet.
- `src/lib.rs` — добавить nested routing для табов, импортировать tabs.
- `public/styles.scss` — добавить стили `.tab-nav` / `.tab-nav-link`; `.editor-grid` больше не использовать глобально (переиспользуется только в `StatsTab`).
- `locales/en/main.ftl`, `locales/ru/main.ftl` — 5 новых i18n-ключей `tab-*`.

---

## Task 1: TabNav component

**Files:**
- Create: `src/components/tab_nav.rs`
- Modify: `src/components/mod.rs`

- [ ] **Step 1: Создать компонент**

Create `src/components/tab_nav.rs`:
```rust
use leptos::prelude::*;
use leptos_router::components::A;

use crate::components::icon::Icon;

#[derive(Clone)]
pub struct TabItem {
    pub path: &'static str,
    pub label: Signal<String>,
    pub icon: &'static str,
}

#[component]
pub fn TabNav(#[prop(into)] base: Signal<String>, items: Vec<TabItem>) -> impl IntoView {
    view! {
        <nav class="tab-nav">
            {items
                .into_iter()
                .map(|item| {
                    let path = item.path;
                    let href = move || format!("{}/{}", base.get(), path);
                    view! {
                        <A href=href attr:class="tab-nav-link">
                            <Icon name=item.icon size=16 />
                            <span class="tab-nav-label">{item.label}</span>
                        </A>
                    }
                })
                .collect_view()}
        </nav>
    }
}
```

- [ ] **Step 2: Зарегистрировать модуль**

Edit `src/components/mod.rs` — добавить строку после `pub mod sync_indicator;` (алфавитно):
```rust
pub mod tab_nav;
```

- [ ] **Step 3: Проверить сборку**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished` без ошибок.

---

## Task 2: Локализация

**Files:**
- Modify: `locales/en/main.ftl`
- Modify: `locales/ru/main.ftl`

- [ ] **Step 1: Английские ключи**

Добавить в конец `locales/en/main.ftl` (или рядом с другими `tab-*`/`view-*` ключами, найти место через `grep -n view-editor locales/en/main.ftl`):
```
tab-stats = Stats
tab-features = Features
tab-magic = Magic
tab-inventory = Inventory
tab-backstory = Backstory
```

- [ ] **Step 2: Русские ключи**

Добавить в `locales/ru/main.ftl` аналогично:
```
tab-stats = Характеристики
tab-features = Особенности
tab-magic = Магия
tab-inventory = Инвентарь
tab-backstory = История
```

- [ ] **Step 3: Проверить сборку**

Run: `cargo build 2>&1 | tail -3`
Expected: `Finished` без ошибок.

---

## Task 3: Tab components — Stats

**Files:**
- Create: `src/pages/character/tabs/mod.rs`
- Create: `src/pages/character/tabs/stats.rs`

- [ ] **Step 1: Создать stats.rs**

Create `src/pages/character/tabs/stats.rs`:
```rust
use leptos::prelude::*;

use crate::components::panels::{
    ability_scores::AbilityScoresPanel, combat::CombatPanel,
    proficiencies::ProficienciesPanel, saving_throws::SavingThrowsPanel, skills::SkillsPanel,
};

#[component]
pub fn StatsTab() -> impl IntoView {
    view! {
        <div class="editor-grid">
            <div class="editor-column editor-column-left">
                <AbilityScoresPanel />
                <SavingThrowsPanel />
                <SkillsPanel />
                <ProficienciesPanel />
            </div>
            <div class="editor-column editor-column-center">
                <CombatPanel />
            </div>
        </div>
    }
}
```

- [ ] **Step 2: Создать mod.rs модуля tabs**

Create `src/pages/character/tabs/mod.rs`:
```rust
pub mod backstory;
pub mod features;
pub mod inventory;
pub mod magic;
pub mod stats;
```

(Пока только stats файл будет, остальные создадутся дальше — модуль регистрирует их заранее, чтобы следующие задачи только добавляли файлы.)

- [ ] **Step 3: Временно закомментировать несуществующие модули**

Чтобы сборка прошла после Task 3, закомментировать в `mod.rs` все строки кроме `pub mod stats;`:
```rust
// pub mod backstory;
// pub mod features;
// pub mod inventory;
// pub mod magic;
pub mod stats;
```

Раскомментируем в соответствующих задачах.

---

## Task 4: Tab components — Features

**Files:**
- Create: `src/pages/character/tabs/features.rs`
- Modify: `src/pages/character/tabs/mod.rs`

- [ ] **Step 1: Создать features.rs**

Create `src/pages/character/tabs/features.rs`:
```rust
use leptos::prelude::*;

use crate::components::panels::{
    class_fields::ClassFieldsPanels, features::FeaturesPanel,
};

#[component]
pub fn FeaturesTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <ClassFieldsPanels />
            <FeaturesPanel />
        </div>
    }
}
```

- [ ] **Step 2: Раскомментировать в mod.rs**

Edit `src/pages/character/tabs/mod.rs` — раскомментировать `pub mod features;`.

---

## Task 5: Tab components — Magic

**Files:**
- Create: `src/pages/character/tabs/magic.rs`
- Modify: `src/pages/character/tabs/mod.rs`

- [ ] **Step 1: Создать magic.rs**

Create `src/pages/character/tabs/magic.rs`:
```rust
use leptos::prelude::*;

use crate::components::panels::spellcasting::SpellcastingPanel;

#[component]
pub fn MagicTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <SpellcastingPanel />
        </div>
    }
}
```

- [ ] **Step 2: Раскомментировать в mod.rs**

Edit `src/pages/character/tabs/mod.rs` — раскомментировать `pub mod magic;`.

---

## Task 6: Tab components — Inventory

**Files:**
- Create: `src/pages/character/tabs/inventory.rs`
- Modify: `src/pages/character/tabs/mod.rs`

- [ ] **Step 1: Создать inventory.rs**

Create `src/pages/character/tabs/inventory.rs`:
```rust
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
```

- [ ] **Step 2: Раскомментировать в mod.rs**

Edit `src/pages/character/tabs/mod.rs` — раскомментировать `pub mod inventory;`.

---

## Task 7: Tab components — Backstory

**Files:**
- Create: `src/pages/character/tabs/backstory.rs`
- Modify: `src/pages/character/tabs/mod.rs`

- [ ] **Step 1: Создать backstory.rs**

Create `src/pages/character/tabs/backstory.rs`:
```rust
use leptos::prelude::*;

use crate::components::panels::{notes::NotesPanel, personality::PersonalityPanel};

#[component]
pub fn BackstoryTab() -> impl IntoView {
    view! {
        <div class="editor-tab">
            <PersonalityPanel />
            <NotesPanel />
        </div>
    }
}
```

- [ ] **Step 2: Раскомментировать в mod.rs**

Edit `src/pages/character/tabs/mod.rs` — раскомментировать `pub mod backstory;`. Теперь весь файл должен выглядеть так:
```rust
pub mod backstory;
pub mod features;
pub mod inventory;
pub mod magic;
pub mod stats;
```

- [ ] **Step 3: Зарегистрировать tabs в pages/character/mod.rs**

Edit `src/pages/character/mod.rs` — добавить строку:
```rust
pub mod tabs;
```

(Сохраняя алфавитный порядок по существующим модулям.)

- [ ] **Step 4: Проверить сборку**

Run: `cargo build 2>&1 | tail -5`
Expected: `Finished`, все 5 tab-компонентов компилируются.

---

## Task 8: Переделать CharacterEditor в layout

**Files:**
- Modify: `src/pages/character/editor.rs`

- [ ] **Step 1: Полностью переписать editor.rs**

Replace `src/pages/character/editor.rs` с:
```rust
use leptos::prelude::*;
use leptos_fluent::move_tr;
use leptos_router::{hooks::use_params, nested_router::Outlet, params::Params};
use uuid::Uuid;

use crate::{
    BASE_URL,
    components::{
        character_header::CharacterHeader,
        tab_nav::{TabItem, TabNav},
    },
};

#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct EditorParams {
    id: Uuid,
}

#[component]
pub fn CharacterEditor() -> impl IntoView {
    let params = use_params::<EditorParams>();
    let base = Signal::derive(move || {
        params
            .get()
            .ok()
            .map(|p| format!("{BASE_URL}/c/{}", p.id))
            .unwrap_or_default()
    });

    let items = vec![
        TabItem {
            path: "stats",
            label: move_tr!("tab-stats"),
            icon: "scroll-text",
        },
        TabItem {
            path: "features",
            label: move_tr!("tab-features"),
            icon: "sparkles",
        },
        TabItem {
            path: "magic",
            label: move_tr!("tab-magic"),
            icon: "wand",
        },
        TabItem {
            path: "inventory",
            label: move_tr!("tab-inventory"),
            icon: "backpack",
        },
        TabItem {
            path: "backstory",
            label: move_tr!("tab-backstory"),
            icon: "book-open",
        },
    ];

    view! {
        <CharacterHeader />
        <TabNav base=base items=items />
        <Outlet />
    }
}
```

- [ ] **Step 2: Проверить сборку (упадёт)**

Run: `cargo build 2>&1 | tail -5`
Expected: FAIL — editor теперь выдаёт `Outlet`, но в роутере ещё нет вложенных routes. Допустимо — исправим в Task 9.

---

## Task 9: Обновить routing

**Files:**
- Modify: `src/lib.rs`

- [ ] **Step 1: Добавить импорты и Redirect-компонент**

Edit `src/lib.rs`:

Добавить в `use leptos_router::...`:
```rust
use leptos_router::{
    components::{ParentRoute, Redirect, Route, Router, Routes},
    hooks::use_params,
    params::Params,
    path,
};
```

Добавить в `use pages::...` (в блоке `character::...`):
```rust
use pages::{
    character::{
        editor::CharacterEditor, layout::CharacterLayout, list::CharacterList,
        quick_start::QuickStart, session::CharacterSession, story::CharacterStory,
        tabs::{
            backstory::BackstoryTab, features::FeaturesTab, inventory::InventoryTab,
            magic::MagicTab, stats::StatsTab,
        },
    },
    // ... остальное без изменений
};
```

- [ ] **Step 2: Добавить RedirectToStats helper**

Добавить в `src/lib.rs` перед `pub fn App`:
```rust
#[derive(Params, Clone, Debug, PartialEq, Eq)]
struct IdParam {
    id: uuid::Uuid,
}

#[component]
fn RedirectToStats() -> impl IntoView {
    let params = use_params::<IdParam>();
    move || {
        params.get().ok().map(|p| {
            let path = format!("{BASE_URL}/c/{}/stats", p.id);
            view! { <Redirect path=path /> }
        })
    }
}
```

- [ ] **Step 3: Заменить CharacterEditor route на ParentRoute с табами**

В `src/lib.rs` внутри `<Routes>`, заменить:
```rust
<ParentRoute path=path!("/c/:id") view=CharacterLayout>
    <Route path=path!("") view=CharacterEditor />
    <Route path=path!("/session") view=CharacterSession />
    <Route path=path!("/quick-start") view=QuickStart />
    <Route path=path!("/story") view=CharacterStory />
    <Route path=path!("/story/:story_id") view=CharacterStory />
</ParentRoute>
```

На:
```rust
<ParentRoute path=path!("/c/:id") view=CharacterLayout>
    <ParentRoute path=path!("") view=CharacterEditor>
        <Route path=path!("") view=RedirectToStats />
        <Route path=path!("/stats") view=StatsTab />
        <Route path=path!("/features") view=FeaturesTab />
        <Route path=path!("/magic") view=MagicTab />
        <Route path=path!("/inventory") view=InventoryTab />
        <Route path=path!("/backstory") view=BackstoryTab />
    </ParentRoute>
    <Route path=path!("/session") view=CharacterSession />
    <Route path=path!("/quick-start") view=QuickStart />
    <Route path=path!("/story") view=CharacterStory />
    <Route path=path!("/story/:story_id") view=CharacterStory />
</ParentRoute>
```

- [ ] **Step 4: Проверить сборку**

Run: `cargo build 2>&1 | tail -10`
Expected: `Finished` без ошибок.

---

## Task 10: CSS стили табов

**Files:**
- Modify: `public/styles.scss`

- [ ] **Step 1: Добавить стили .tab-nav**

Edit `public/styles.scss`, добавить перед секцией `/* --------------------- Editor Grid Layout ---------------------` (строка ~855):
```scss
/* --------------------- Tab Navigation ------------------------ */
.tab-nav {
  display: flex;
  gap: var(--size-2);
  border-bottom: 1px solid var(--panel-border);
  margin-bottom: var(--size-3);
  overflow-x: auto;
}

.tab-nav-link {
  display: inline-flex;
  align-items: center;
  gap: var(--size-2);
  padding: var(--size-2) var(--size-3);
  border-bottom: 2px solid transparent;
  color: var(--text-secondary);
  text-decoration: none;
  white-space: nowrap;
  margin-bottom: -1px;

  &[aria-current="page"] {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  &:hover {
    color: var(--text-primary);
  }
}

.editor-tab {
  display: flex;
  flex-direction: column;
  gap: var(--size-3);
}

@media (max-width: 600px) {
  .tab-nav-label {
    display: none;
  }
}
```

- [ ] **Step 2: Проверить сборку**

Run: `cargo build 2>&1 | tail -3`
Expected: `Finished`.

- [ ] **Step 3: Форматирование**

Run: `cargo +nightly fmt`

- [ ] **Step 4: Clippy**

Run: `cargo clippy --no-deps 2>&1 | tail -10`
Expected: нет новых warnings (кроме уже существующего про `ConfirmButton` dead_code).

---

## Task 11: Verify in browser

**Files:** (ручное тестирование)

- [ ] **Step 1: Запустить dev server**

Run (в отдельном терминале): `trunk serve --port 3000`

- [ ] **Step 2: Проверить redirect**

Открыть в браузере `http://localhost:3000/c/{uuid}` существующего персонажа.
Expected: автоматический редирект на `/c/{uuid}/stats`.

- [ ] **Step 3: Переключение табов**

Кликнуть по каждой вкладке:
- Stats → 3-column grid (на desktop)
- Features → 1-column
- Magic → 1-column
- Inventory → 1-column
- Backstory → 1-column

Expected: URL меняется, CharacterHeader остаётся видимым, содержимое переключается без перезагрузки.

- [ ] **Step 4: Deep link**

Открыть прямую ссылку `http://localhost:3000/c/{uuid}/magic` — должен сразу открыть Magic tab с подсветкой в nav.

- [ ] **Step 5: Back-button**

Переключить 3 таба, нажать Back в браузере. Expected: возврат к предыдущему табу.

- [ ] **Step 6: Mobile viewport**

В DevTools переключить viewport на 375×667 (iPhone SE).
Expected:
- Tab nav скроллится горизонтально, лейблы скрыты (только иконки) на ≤600px.
- Stats вкладка — одна колонка (существующий `.editor-grid @ max-width: 600px`).

- [ ] **Step 7: Активная подсветка**

На каждой вкладке активный `<a>` в `.tab-nav` должен иметь:
- Цвет текста и нижнего border — `var(--accent)` (проверить в DevTools computed styles).

- [ ] **Step 8: Темы**

Переключить системную тему (Dev Tools → Rendering → Emulate CSS media feature `prefers-color-scheme`). Expected: табы корректно стилизованы в обеих темах.

---

## Task 12: Commit

**Files:** (все изменения)

- [ ] **Step 1: Проверить git status**

Run: `git status --short`
Expected: видны все созданные/изменённые файлы.

- [ ] **Step 2: Stage**

Run:
```bash
git add src/components/tab_nav.rs src/components/mod.rs \
        src/pages/character/tabs/ src/pages/character/mod.rs \
        src/pages/character/editor.rs src/lib.rs \
        public/styles.scss locales/en/main.ftl locales/ru/main.ftl
```

- [ ] **Step 3: Commit**

Run:
```bash
git commit -m "$(cat <<'EOF'
feat: split character editor into 5 tabs with URL routing

Replace the single-page 3-column editor with a tabbed layout (Stats,
Features, Magic, Inventory, Backstory) served on /c/:id/<tab>. Each
tab composes the existing panels without modification; Stats keeps
the 3-column grid, others use a single vertical stack.

A new reusable TabNav component drives navigation via
leptos_router <A>, getting active-tab highlighting from aria-current
set by the router.
EOF
)"
