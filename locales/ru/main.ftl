## Страницы
page-characters = Персонажи D&D 5e
btn-new-character = + Новый персонаж
btn-load-character = Загрузить из файла
page-not-found = Страница не найдена
character-not-found = Персонаж не найден
back-to-list = Назад к списку персонажей
btn-delete = Удалить
btn-cancel = Отмена
confirm-delete = Удалить этого персонажа?

## Заголовок персонажа
character-name = Имя персонажа
species = Раса
background = Предыстория
alignment = Мировоззрение
xp = Опыт
total-level = Общий уровень
prof-bonus = Бонус мастерства
classes = Классы
class = Класс
subclass = Подкласс
btn-add-class = + Добавить / повысить
btn-edit-feature = Редактировать параметры способности
apply = Применить
apply-features-title = Применить способности
build-replay-hint-title = Эти способности отредактированы или не применены. Нажмите Перестроить чтобы пересчитать персонажа.
build-choice-hint-title = В этих способностях нужно выбрать опцию:
build-pending-apply-title = Новые уровни или изменения ещё не применены. Нажмите Применить, чтобы добавить связанные способности.
build-needs-rebuild-title = У персонажа {$reasons}. Нужна пересборка.
rebuild-reason-species = изменена раса
rebuild-reason-background = изменена предыстория
rebuild-reason-class-removed = удалён класс «{$class}»
rebuild-reason-level-lowered = понижен уровень класса «{$class}» (было {$applied}, стало {$current})
rebuild-reason-subclass-changed = изменён подкласс класса «{$class}»
rebuild-reason-feature-removed = удалена особенность «{$name}»
rebuild-reason-legacy-system-markers = устаревший формат данных
replace-with-feat = Заменить на…
no-eligible-options = Нет доступных вариантов
export-json = Сохранить в файл
import-json = Загрузить из файла
reset-character = Сбросить персонажа
actions-menu = Действия
share-link = Поделиться ссылкой
share-toggle = Публичный доступ
share-loading = Загрузка персонажа...
share-not-found = Персонаж не найден или не опубликован
hint-no-characters = Не видите своих персонажей? Без входа в облако они доступны только на этом устройстве и могут быть потеряны при очистке данных браузера. Войдите через Google для синхронизации между устройствами.
hint-character-not-found = Это ваш персонаж на другом аккаунте? Войдите через Google, чтобы открыть его.
hint-sign-in-button = Войти через Google
toast-signin-prompt = Ваши персонажи доступны только на этом устройстве и могут быть потеряны при очистке данных браузера.
toast-signin-action = Войти
toast-dismiss = Закрыть
toast-export-copied = JSON персонажа скопирован в буфер обмена. Telegram Mini App не умеет сохранять файлы — вставьте в Сохранённые Сообщения (или любой чат), чтобы сохранить данные.
toast-export-copy-failed = Не удалось скопировать JSON персонажа в буфер обмена.
toast-sync-error = Ошибка синхронизации: { $error }
toast-login-tma = Вход через Google не работает внутри Telegram. Ссылка скопирована — вставьте в браузер, чтобы войти.
update-available = Доступна новая версия.
update-button-reload = Обновить
copy-character = Копировать персонажа
import-conflict-title = Персонаж уже существует
import-conflict-message = У вас уже есть более новая версия «{$name}». Импорт перезапишет её. Различия указаны ниже.
import-anyway = Импортировать
import-as-copy = Импорт как копию
import-cancel = Отмена
diff-section-identity = Общие сведения
diff-field = Поле
diff-local = Локальный
diff-imported = Импортируемый
diff-no-differences = Видимых различий нет
no-class = Нет класса
level-prefix = Уровень

## Названия панелей
panel-ability-scores = Характеристики
panel-saving-throws = Спасброски
panel-skills = Навыки
panel-damage-modifiers = Модификаторы урона
saving-throw = Спасбросок
panel-combat = Бой
panel-spellcasting = Заклинания
panel-equipment = Снаряжение
panel-features = Способности
panel-personality = Личность
panel-proficiencies = Владения и языки
panel-notes = Заметки
btn-add-note = + Добавить заметку
diff-notes-summary = «{ $text }» · { $level } · { $date } · всего: { $count }

## Панель боя
armor-class = Класс защиты
recalculate = Пересчитать
rebuild = Пересобрать
rebuild-confirm = Персонаж будет пересобран с нуля. HP, использованные слоты и hit dice сохранятся.
toast-rebuild-done = Персонаж пересобран
toast-rebuild-skipped = { $count ->
    [one] Пересборка завершена; { $count } неизвестная способность сохранена без изменений. Откройте вкладку Билд, чтобы проверить.
    [few] Пересборка завершена; { $count } неизвестные способности сохранены без изменений. Откройте вкладку Билд, чтобы проверить.
   *[many] Пересборка завершена; { $count } неизвестных способностей сохранено без изменений. Откройте вкладку Билд, чтобы проверить.
}
toast-rebuild-removed = { $count ->
    [one] Пересборка завершена; { $count } устаревшая способность удалена (нет в текущих правилах).
    [few] Пересборка завершена; { $count } устаревшие способности удалены (нет в текущих правилах).
   *[many] Пересборка завершена; { $count } устаревших способностей удалено (нет в текущих правилах).
}
toast-rebuild-skipped-and-removed = Пересборка завершена: { $skipped ->
    [one] { $skipped } неизвестная сохранена
    [few] { $skipped } неизвестные сохранены
   *[many] { $skipped } неизвестных сохранено
}, { $removed ->
    [one] { $removed } устаревшая удалена
    [few] { $removed } устаревшие удалены
   *[many] { $removed } устаревших удалено
}. Откройте вкладку Билд, чтобы проверить.
toast-rebuild-failed-class = Не удалось пересобрать: отсутствует определение класса «{ $name }». Выберите класс заново в шапке персонажа.
toast-rebuild-failed-species = Не удалось пересобрать: отсутствует определение расы «{ $name }». Выберите расу заново в шапке персонажа.
toast-rebuild-failed-background = Не удалось пересобрать: отсутствует определение предыстории «{ $name }». Выберите предысторию заново в шапке персонажа.
toast-rebuild-failed-multiclass = Не удалось пересобрать: не выполнены требования для мультикласса «{ $class }». Поднимите нужную характеристику через Generation или ASI во вкладке Билд.
toast-rebuild-action-open-build = Билд
initiative = Инициатива
speed = Скорость
attack-count = Атаки
inspiration = Вдохновение
proficiency-bonus = Бонус мастерства
level = Уровень
class-level = Уровень класса
hit-dice = Кости здоровья
hit-dice-max = Кости здоровья (макс)
hit-dice-used = Кости здоровья (потрачено)
hit-dice-sides = Размер кости здоровья
caster-level = Уровень заклинателя
caster-ability = Заклинательная характеристика
caster-coef = Тип заклинателя
caster-coef-full = Полный
caster-coef-half = Половинный
caster-coef-third = Третичный
spell-slot = Ячейка
spell-slot-used = Потрачено ячеек
spell-slot-pool = Тип ячеек
spell-cantrips = Заговоры
spell-known = Известные заклинания
spell-ready = Подготовленные заклинания
hp = Очки здоровья
current-hp = Текущие ОЗ
hp-max = Макс. ОЗ
temp-hp = Врем. ОЗ
successes = Успехи
failures = Провалы
short-rest = Короткий отдых
long-rest = Длинный отдых
drop-concentration = Сбросить концентрацию
reset-stats = Сброс

## Панель заклинаний
casting-ability = Характеристика
spell-save-dc = Сложность
spell-attack = Атака
spell-slots = Ячейки заклинаний
spells = Заклинания
spellbook = Книга заклинаний
prepared-spells = Подготовленные заклинания
spell-name = Название заклинания
free-uses = Своб. исп.

## Панель снаряжения
weapons = Оружие
name = Название
atk-magic = Магия
weapon-ability = Характеристика
attack = Атака
damage = Урон
heal = Лечение
btn-add-weapon = + Добавить оружие
btn-add-effect = Добавить эффект
armor = Доспехи
base-ac = Базовый КЗ
ac-formula = Формула КЗ
btn-add-armor = + Добавить доспех
armor-type-light = Лёгкий
armor-type-medium = Средний
armor-type-heavy = Тяжёлый
armor-type-shield = Щит
armor-type-natural = Естественный
weapon-category-simple = Простое
weapon-category-martial = Воинское
items = Предметы
item-name = Название предмета
qty = Кол-во
description = Описание
btn-add-item = + Добавить предмет
currency = Валюта
spend = Потратить
cast = Сотворить
gain = Получить
add-item = Добавить предмет

## Способности / Личность / Владения
feature-name = Название способности
btn-add-feature = Добавить способность
source-class = Класс
source-subclass = Подкласс
source-species = Раса
source-background = Предыстория
source-user = Вручную
history = Предыстория
personality-traits = Черты характера
ideals = Идеалы
bonds = Привязанности
flaws = Слабости
proficiencies = Владения
languages = Языки
language = Язык
btn-add-language = + Добавить язык
used = Исп.
total = Всего
max = Макс.
cost = Цена
btn-add-spell = + Добавить заклинание
choose-option = Выбрать
search = Поиск…
browse-options = Обзор вариантов
btn-add-option = + Добавить вариант

## Характеристики
ability-strength = Сила
ability-dexterity = Ловкость
ability-constitution = Телосложение
ability-intelligence = Интеллект
ability-wisdom = Мудрость
ability-charisma = Харизма
ability-str = СИЛ
ability-dex = ЛОВ
ability-con = ТЕЛ
ability-int = ИНТ
ability-wis = МУД
ability-cha = ХАР

## Навыки
skill-acrobatics = Акробатика
skill-animal-handling = Уход за животными
skill-arcana = Магия
skill-athletics = Атлетика
skill-deception = Обман
skill-history = История
skill-insight = Проницательность
skill-intimidation = Запугивание
skill-investigation = Расследование
skill-medicine = Медицина
skill-nature = Природа
skill-perception = Восприятие
skill-performance = Выступление
skill-persuasion = Убеждение
skill-religion = Религия
skill-sleight-of-hand = Ловкость рук
skill-stealth = Скрытность
skill-survival = Выживание

## Мировоззрения
alignment-lawful-good = Законно-добрый
alignment-neutral-good = Нейтрально-добрый
alignment-chaotic-good = Хаотично-добрый
alignment-lawful-neutral = Законно-нейтральный
alignment-true-neutral = Истинно нейтральный
alignment-chaotic-neutral = Хаотично-нейтральный
alignment-lawful-evil = Законно-злой
alignment-neutral-evil = Нейтрально-злой
alignment-chaotic-evil = Хаотично-злой

## Владения
prof-light-armor = Лёгкие доспехи
prof-medium-armor = Средние доспехи
prof-heavy-armor = Тяжёлые доспехи
prof-shields = Щиты
prof-simple-weapons = Простое оружие
prof-martial-weapons = Воинское оружие

## Типы урона
damage-acid = Кислота
damage-bludgeoning = Дробящий
damage-cold = Холод
damage-fire = Огонь
damage-force = Чистая сила
damage-lightning = Электричество
damage-necrotic = Некротический
damage-piercing = Колющий
damage-poison = Яд
damage-psychic = Психический
damage-radiant = Излучение
damage-slashing = Рубящий
damage-thunder = Звук

## Диалоги подтверждения
confirm-reset = Сбросить персонажа до пустого?
remove-class = Удалить класс
confirm-remove-class = Удалить этот класс и все его уровни? Потом нужно будет пересобрать персонажа.

## Сессия
slot-level = Ур. { $level }
session-actions = Заклинания и оружие
session-stats = Основные показатели
session-backpack = Рюкзак
session-resources = Ресурсы
view-session = Сессия
view-editor = Редактор
view-story = История
tab-stats = Статы
tab-build = Билд
tab-magic = Магия
tab-inventory = Инвентарь
tab-backstory = История
story-new = Новая история
story-prompt-placeholder = Опишите, что произошло между сессиями...
story-generate = Сгенерировать
story-stop = Стоп
story-no-api-key = Настройте API-ключ для генерации историй.
story-settings = Настройки AI
story-api-key = API-ключ
story-get-key = (получить)
story-model = Модель чата
ai-settings-image-model = Модель изображений
ai-settings-fetch-failed = Не удалось загрузить список моделей
ai-settings-provider-hosted = Использовать общий AI (нужен вход через Google)
ai-settings-google-required = Войдите через Google, чтобы пользоваться общим AI, или снимите галку и укажите свой ключ.
story-save = Сохранить
story-delete = Удалить
story-copy = Копировать
story-error = Ошибка генерации
story-select = Выберите историю или создайте новую
story-retry = Повторить
ai-generate-title = AI генератор персонажа
ai-generate-description = Опишите персонажа
ai-generate-placeholder = Мрачный полуэльф-следопыт, выросший в дикой глуши...
ai-generate-button = AI генерация
ai-generate-no-key = Настройте API ключ в настройках (шестерёнка внизу) для использования AI генерации.
ai-generate-phase-identity = Выбираю расу, класс...
ai-generate-phase-choices = Заполняю варианты выбора...
ai-generate-phase-retry = Исправляю выбор...
ai-generate-error = Ошибка генерации
level-up = Повысить уровень
level-up-choose-class = Выберите класс для повышения
session-cantrips = Заговоры
session-no-weapons = Нет оружия
session-no-items = Нет предметов
session-ability-mods = Модификаторы характеристик
session-saving-throws = Спасброски
session-languages = Понятные языки
session-damage-modifiers = Сопротивления
damage-vulnerability = Уязвимость
damage-resistance = Сопротивление
damage-immunity = Иммунитет
damage-reduction = ПУ
action-type-action = Действие
action-type-bonus-action = Бонусное действие
action-type-reaction = Реакция
session-effects = Активные эффекты
effect-add = Добавить эффект
effect-remove = Удалить эффект
effect-name = Название эффекта
effect-expr = Выражение (опционально)
effect-dice = Кости
effect-reroll = Перебросить кости
roll-all-dice = Бросить все кости
dice-rolls-title = Броски костей
btn-confirm = Подтвердить
apply-effect = Применить эффект

## Справочные страницы
ref-reference = Справочник
ref-classes = Классы
ref-species = Расы
ref-backgrounds = Предыстории
ref-level = Уровень
ref-features = Способности
ref-hit-die = Кость хитов
ref-cantrips = Заговоры
ref-spells-known = Известно
ref-spells-ready = Готово
ref-subclasses = Подклассы
ref-progression = Прогрессия
ref-select-class = Выберите класс для просмотра
ref-select-species = Выберите расу для просмотра
ref-select-background = Выберите предысторию для просмотра
ref-search-feature = Поиск способностей...
feat-cat-class = Классовые способности
feat-cat-origin = Черты происхождения
feat-cat-general = Общие черты
feat-cat-fighting-style = Боевые стили
feat-cat-epic-boon = Эпические благословения
feat-cat-generation = Генерация
feat-cat-faction = Фракция
feat-cat-dragonmark = Знак дракона
feat-cat-system-species = Раса (системная)
feat-cat-system-background = Предыстория (системная)
feat-cat-system-subclass = Подкласс (системный)
feat-cat-system-class = Класс (системный)
feat-cat-all = Все категории
ref-spells = Заклинания
ref-select-spell-list = Выберите список заклинаний для просмотра
ref-cantrips-level = заговоры
ref-spell-level = {$level}-й уровень
ref-spell-min-level = с {$level}-го уровня
ref-spell-always-ready = всегда подготовлено
ref-spell-cast-time = Время накладывания
ref-spell-range = Дистанция
ref-spell-duration = Длительность
ref-spell-concentration = Концентрация
ref-spell-ritual = Ритуал
ref-spell-range-self = На себя
ref-spell-range-touch = Касание
ref-spell-range-feet = {$feet} фт.
ref-spell-duration-instant = Мгновенное
ref-spell-duration-rounds = {$rounds} {$rounds ->
    [one] раунд
    [few] раунда
   *[other] раундов
}
ref-spell-duration-minutes = {$minutes} мин.
ref-spell-duration-hours = {$hours} ч.
ref-spell-duration-forever = Пока не рассеется
ref-spell-cast-rounds = {$rounds} раундов
ref-spell-cast-minutes = {$minutes} мин.
ref-spell-cast-hours = {$hours} ч.
ref-spell-category = Категория
spell-cat-damage = Урон
spell-cat-healing = Лечение
spell-cat-buff = Усиление
spell-cat-debuff = Ослабление
spell-cat-control = Контроль
spell-cat-defense = Защита
spell-cat-utility = Утилита
spell-cat-summon = Призыв
spell-cat-social = Социальное
ref-prerequisites = Требования
ref-spell-list-link = Список заклинаний
expr-and = и
expr-or = или
expr-not = не

## Пулы ячеек заклинаний
pool-arcane = Магические ячейки
pool-pact = Ячейки пакта

## Быстрый старт
quick-start-title = Быстрый старт
quick-start-generation = Характеристики
quick-start-create = Создать персонажа
quick-start-skip = Пропустить


## Облачная синхронизация
sync-disabled = Офлайн
sync-connecting = Подключение...
sync-synced = Синхронизировано
sync-syncing = Синхронизация...
sync-error = Ошибка синхронизации
sync-sign-in-google = Войти через Google
show-expression = Показать выражение
points = Очки
points-max = Макс. очков
die-sides = Граней куба
die-count = Число костей
die-used = Использовано костей
bonus = Бонус
choice-count = Количество выборов
sticky = Всегда подготовлено
free-uses-used = Использовано своб.
reset = Сброс
spell-level-badge = { $count ->
    [0] Заговор
   *[other] { $count } ур.
}

## Портрет
avatar-change = Сменить портрет
avatar-remove = Удалить портрет
avatar-generate = Сгенерировать через AI
avatar-load-failed = Не удалось загрузить изображение
avatar-generate-title = Сгенерировать портрет
avatar-generate-description = Дополнительные детали (необязательно)
avatar-generate-placeholder = поза, одежда, настроение…
avatar-generate-button = Сгенерировать
avatar-generate-phase-rendering = Рисую портрет…
avatar-generate-failed = Не удалось сгенерировать портрет
avatar-close = Закрыть

# Enchantment modal
enchantment-edit = Редактировать чары
enchantment-charges = Заряды
enchantment-charges-used = Потрачено
enchantment-charges-max = Максимум
enchantment-passives = Пассивные эффекты
enchantment-actions = Действия
enchantment-no-passives = Нет пассивных эффектов
enchantment-no-actions = Нет действий
enchantment-action-name = Название действия
enchantment-option-name = Название режима
assign-when = Когда
action-type = Тип
effects = Эффекты
effect-range = Дальность
effect-duration = Длительность
effect-scope = Скоуп
effect-stackable = Складывается
range-caster = На себя
range-touch = Касание
range-feet = Футы
duration-instant = Мгновенно
duration-rounds = Раунды
duration-forever = Постоянно
charges = Заряды
charges-max = Заряды (макс)
charges-used = Заряды (потрачено)
quantity = Количество
equipped = Надето
session-gear-actions = Действия предметов
choice-cost = Заряды
choice-consumes = Расход
btn-add-passive = + Добавить эффект
btn-add-action = + Добавить действие
btn-save = Сохранить
when-on-gear-active = Пока активно
when-on-effect = На эффекте
when-on-long-rest = При долгом отдыхе
when-on-short-rest = При коротком отдыхе
