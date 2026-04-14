# Character Editor — табы

## Context

Сейчас `CharacterEditor` рендерит все 11 панелей в жёстком 3-колоночном grid'е. Страница получилась очень длинной, scroll утомителен, все панели собраны в одну плоскость без визуальной группировки. Пользователь хочет разбить редактор на 5 тематических вкладок, с URL-роутингом (deep-link, браузерный back-button).

## Решение

Заменить одностраничный `CharacterEditor` иерархическим routing'ом: `CharacterEditor` превращается в layout c `CharacterHeader`, `TabNav` и `<Outlet/>`; каждая вкладка — отдельный тонкий компонент-композиция существующих панелей.

### Routing

```
/c/:id  ← CharacterLayout (не меняется)
  ""             → CharacterEditor (ParentRoute: header + tab nav + <Outlet/>)
    ""           → <Redirect to="stats">
    /stats       → StatsTab
    /features    → FeaturesTab
    /magic       → MagicTab
    /inventory   → InventoryTab
    /backstory   → BackstoryTab
  /session       → CharacterSession (без изменений)
  /quick-start   → QuickStart
  /story, /story/:story_id → CharacterStory
```

Голый `/c/:id` перенаправляет на `/c/:id/stats`. Deep-link на любую вкладку работает, back-button пролистывает историю вкладок.

### Компоненты

- **`src/components/tab_nav.rs`** — новый универсальный компонент. API:
  ```rust
  pub struct TabItem {
      pub path: &'static str,   // "stats", "features", ...
      pub label: &'static str,  // i18n key
      pub icon: &'static str,   // Lucide name
  }

  #[component]
  pub fn TabNav(base: String, items: Vec<TabItem>) -> impl IntoView
  ```
  Рендерит `<nav class="tab-nav">` c `<A href="{base}/{path}" attr:class="tab-nav-link">`. Leptos router сам добавит `aria-current="page"` на активную ссылку — стилизация подчёркивания через этот атрибут.

- **`src/pages/character/tabs/mod.rs`** — модуль с 5 тонкими tab-компонентами. Каждый — `#[component] pub fn StatsTab() -> impl IntoView { view! { ... } }`. Импортирует и композирует существующие панели, ничего не меняя в них.

- **`src/pages/character/editor.rs`** — переделан из страницы в layout:
  ```rust
  pub fn CharacterEditor() -> impl IntoView {
      let id = /* ... */;
      let base = format!("{BASE_URL}/c/{id}");
      view! {
          <CharacterHeader />
          <TabNav base=base items=TAB_ITEMS.to_vec() />
          <Outlet />
      }
  }
  ```

### Распределение панелей

| Tab       | Панели | Layout |
|-----------|--------|--------|
| Stats     | AbilityScoresPanel, SavingThrowsPanel, SkillsPanel, ProficienciesPanel, CombatPanel | 3-column grid (desktop), 1-col (mobile) |
| Features  | ClassFieldsPanels, FeaturesPanel | 1 column |
| Magic     | SpellcastingPanel | 1 column |
| Inventory | EquipmentPanel | 1 column |
| Backstory | PersonalityPanel, NotesPanel | 1 column |

Текущий `.editor-grid` class переиспользуется только для Stats. Остальные вкладки — простой вертикальный стек (`display: flex; flex-direction: column`).

### Локализация

Новые i18n-ключи в `locales/{en,ru}/main.ftl`:
- `tab-stats = Stats / Характеристики`
- `tab-features = Features / Особенности`
- `tab-magic = Magic / Магия`
- `tab-inventory = Inventory / Инвентарь`
- `tab-backstory = Backstory / История`

### Иконки

Lucide, все уже есть в `public/icons.svg` или скачиваются (см. feedback_check_icons):
- Stats — `scroll-text`
- Features — `sparkles` (проверить/скачать)
- Magic — `wand`
- Inventory — `backpack`
- Backstory — `book-open`

Перед коммитом — проверить наличие всех символов через Grep в `icons.svg`, отсутствующие подтянуть с `raw.githubusercontent.com/lucide-icons/lucide/main/icons/<name>.svg`.

### Стили (`public/styles.scss`)

Новый блок `.tab-nav`:
```scss
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

  &[aria-current="page"] {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  &:hover {
    color: var(--text-primary);
  }
}
```

Mobile (< 768px): на iPhone/узком экране nav получает `overflow-x: auto` и скроллится горизонтально если не влезает. Иконки + label остаются; в крайнем случае прячем label и оставляем только иконку через `.navbar-link-label`-подобный приём (если понадобится).

### Что НЕ меняется

- CharacterLayout, CharacterSession, CharacterStory, QuickStart, ImportCharacter.
- Все существующие панели — ни один файл в `src/components/panels/` не изменяется.
- Логика persist открытых/закрытых панелей (`dnd_pc_panel_*`) — работает как раньше.
- Navbar — не трогаем.

## Верификация

1. `cargo clippy` — нет новых предупреждений.
2. `cargo +nightly fmt`.
3. `trunk serve --port 3000` и ручная проверка:
   - Перейти на `/c/:id` — должен увидеть Stats таб (redirect).
   - Клики по табам меняют URL и контент, CharacterHeader остаётся виден.
   - Deep-link `/c/:id/magic` — сразу открывает Magic.
   - Back/Forward в браузере переключает вкладки.
   - Mobile viewport (< 768px) — табы горизонтально скроллятся, Stats вкладка ломается в одну колонку.
   - Темы light/dark — активный таб имеет `--accent` цвет, неактивные `--text-secondary`.
4. `WASM_BINDGEN_USE_BROWSER=1 cargo test --target wasm32-unknown-unknown` — без регрессий.
