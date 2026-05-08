use std::collections::HashMap;

use serde_json::Value;

use crate::model::{Character, DamageType};

/// Latest schema version. Bumped when a new migration step is added.
/// Characters persisted with schema_version >= CURRENT_SCHEMA_VERSION skip
/// the migration loop entirely.
pub const CURRENT_SCHEMA_VERSION: u32 = 4;

/// Migrate legacy string damage_type values to u8 enum representation.
fn migrate_v1(value: &mut Value) {
    fn damage_type_from_name(s: &str) -> Option<DamageType> {
        match s.to_ascii_lowercase().as_str() {
            "acid" => Some(DamageType::Acid),
            "bludgeoning" => Some(DamageType::Bludgeoning),
            "cold" => Some(DamageType::Cold),
            "fire" => Some(DamageType::Fire),
            "force" => Some(DamageType::Force),
            "lightning" => Some(DamageType::Lightning),
            "necrotic" => Some(DamageType::Necrotic),
            "piercing" => Some(DamageType::Piercing),
            "poison" => Some(DamageType::Poison),
            "psychic" => Some(DamageType::Psychic),
            "radiant" => Some(DamageType::Radiant),
            "slashing" => Some(DamageType::Slashing),
            "thunder" => Some(DamageType::Thunder),
            _ => None,
        }
    }

    if let Some(weapons) = value
        .get_mut("equipment")
        .and_then(|e| e.get_mut("weapons"))
        .and_then(|w| w.as_array_mut())
    {
        for weapon in weapons {
            if let Some(dt) = weapon.get("damage_type").and_then(|v| v.as_str()) {
                let new_val = match damage_type_from_name(dt) {
                    Some(d) => Value::Number((d as u8).into()),
                    None => Value::Null,
                };
                weapon["damage_type"] = new_val;
            }
        }
    }
}

/// Migrate flat spell_slots array to BTreeMap keyed by pool.
fn migrate_v2(value: &mut Value) {
    if value
        .get("spell_slots")
        .is_some_and(|slots| slots.is_array())
    {
        let slots = value["spell_slots"].take();
        value["spell_slots"] = serde_json::json!({ "0": slots });
    }
}

/// Migrate string attack_bonus to i32.
fn migrate_v3(value: &mut Value) {
    if let Some(weapons) = value
        .get_mut("equipment")
        .and_then(|e| e.get_mut("weapons"))
        .and_then(|w| w.as_array_mut())
    {
        for weapon in weapons {
            if let Some(s) = weapon.get("attack_bonus").and_then(|v| v.as_str()) {
                let parsed: i32 = s.parse().unwrap_or(0);
                weapon["attack_bonus"] = Value::Number(parsed.into());
            }
        }
    }
}

/// Migrate FeatureValue::Die from string to { die, used } struct.
fn migrate_v4(value: &mut Value) {
    if let Some(feature_data) = value
        .get_mut("feature_data")
        .and_then(|v| v.as_object_mut())
    {
        for entry in feature_data.values_mut() {
            if let Some(fields) = entry.get_mut("fields").and_then(|v| v.as_array_mut()) {
                for field in fields {
                    if let Some(die_str) = field.get("value").and_then(|v| v.get("Die"))
                        && let Some(s) = die_str.as_str()
                    {
                        let s = s.to_string();
                        field["value"] = serde_json::json!({ "Die": { "die": s, "used": 0 } });
                    }
                }
            }
        }
    }
}

/// Migrate broken "Languages" feature to per-species "Languages ({species})".
fn migrate_v5(value: &mut Value) {
    let has_bare_languages = value
        .get("features")
        .and_then(|v| v.as_array())
        .is_some_and(|feats| {
            feats
                .iter()
                .any(|f| f.get("name").and_then(|n| n.as_str()) == Some("Languages"))
        });

    if !has_bare_languages {
        return;
    }

    let species = value
        .get("identity")
        .and_then(|id| id.get("species"))
        .and_then(|s| s.as_str())
        .unwrap_or("")
        .to_string();

    if species.is_empty() {
        return;
    }

    let (lang_feat, correct_langs): (&str, &[&str]) = match species.as_str() {
        "Aasimar" => ("Languages (Aasimar)", &["Common", "Celestial"]),
        "Dragonborn" => ("Languages (Dragonborn)", &["Common", "Draconic"]),
        "Drow" | "High Elf" | "Wood Elf" | "Khoravar" => ("Languages (Elf)", &["Common", "Elvish"]),
        "Dwarf" => ("Languages (Dwarf)", &["Common", "Dwarvish"]),
        "Forest Gnome" | "Rock Gnome" => ("Languages (Gnome)", &["Common", "Gnomish"]),
        "Goliath" => ("Languages (Goliath)", &["Common", "Giant"]),
        "Halfling" => ("Languages (Halfling)", &["Common", "Halfling"]),
        "Human" | "Shifter" | "Warforged" | "Dhampir" => ("Languages (Human)", &["Common"]),
        "Orc" => ("Languages (Orc)", &["Common", "Orc"]),
        "Tiefling" | "Tiefling (Infernal)" => ("Languages (Tiefling)", &["Common", "Infernal"]),
        "Tiefling (Abyssal)" => ("Languages (Tiefling (Abyssal))", &["Common", "Abyssal"]),
        "Tiefling (Chthonic)" => ("Languages (Tiefling (Chthonic))", &["Common", "Sylvan"]),
        "Changeling" => ("Languages (Changeling)", &["Common", "Sylvan"]),
        "Kalashtar" => ("Languages (Kalashtar)", &["Common", "Quori"]),
        "Boggart" => ("Languages (Boggart)", &["Common", "Sylvan"]),
        "Faerie" => ("Languages (Faerie)", &["Common", "Sylvan"]),
        "Flamekin" => ("Languages (Flamekin)", &["Common", "Primordial"]),
        "Lorwyn Changeling" => ("Languages (Lorwyn Changeling)", &["Common", "Sylvan"]),
        "Rimekin" => ("Languages (Rimekin)", &["Common", "Primordial"]),
        _ => return,
    };

    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        features.retain(|f| f.get("name").and_then(|n| n.as_str()) != Some("Languages"));
        features.push(serde_json::json!({"name": lang_feat}));
    }

    let langs: Vec<Value> = correct_langs
        .iter()
        .map(|l| Value::String(l.to_string()))
        .collect();
    value["languages"] = Value::Array(langs);
}

/// Set `applied: true` on all features that lack the field (old characters).
fn migrate_v6(value: &mut Value) {
    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        for feature in features {
            if feature.get("applied").is_none() {
                feature["applied"] = Value::Bool(true);
            }
        }
    }
}

/// Migrate Feature entries: add `source` from FeatureData.source, remove
/// FeatureData.source. Convert FeatureSource::Class(String) to
/// Class(String, u32).
fn migrate_v7(value: &mut Value) {
    let mut sources: HashMap<String, Value> = HashMap::new();
    if let Some(feature_data) = value.get("feature_data").and_then(|v| v.as_object()) {
        for (name, data) in feature_data {
            if let Some(source) = data.get("source") {
                let mut source = source.clone();
                if let Some(obj) = source.as_object_mut()
                    && let Some(class_val) = obj.get("Class")
                    && let Some(class_name) = class_val.as_str()
                {
                    let class_name = class_name.to_string();
                    obj.insert("Class".to_string(), serde_json::json!([class_name, 1]));
                }
                sources.insert(name.clone(), source);
            }
        }
    }

    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        for feature in features {
            if (feature.get("source").is_none() || feature.get("source") == Some(&Value::Null))
                && let Some(name) = feature.get("name").and_then(|v| v.as_str())
            {
                if let Some(source) = sources.get(name) {
                    feature["source"] = source.clone();
                } else {
                    feature["source"] = serde_json::json!({"User": 0});
                }
            }
        }
    }

    if let Some(feature_data) = value
        .get_mut("feature_data")
        .and_then(|v| v.as_object_mut())
    {
        for (_, data) in feature_data.iter_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.remove("source");
                if let Some(args) = obj.remove("args") {
                    let inputs = if let Some(arr) = args.as_array() {
                        let migrated: Vec<Value> = arr
                            .iter()
                            .map(|entry| {
                                if let Some(obj) = entry.as_object()
                                    && let Some(values) = obj.get("values")
                                {
                                    serde_json::json!({"args": values})
                                } else {
                                    entry.clone()
                                }
                            })
                            .collect();
                        Value::Array(migrated)
                    } else {
                        args
                    };
                    obj.insert("inputs".to_string(), inputs);
                }
            }
        }
    }
}

/// Move `inputs` from `feature_data[name].inputs` to `features[i].inputs`.
fn migrate_v8(value: &mut Value) {
    let mut inputs_by_name: HashMap<String, Vec<Value>> = HashMap::new();
    if let Some(feature_data) = value.get("feature_data").and_then(|v| v.as_object()) {
        for (name, data) in feature_data {
            if let Some(inputs) = data.get("inputs").and_then(|v| v.as_array())
                && !inputs.is_empty()
            {
                inputs_by_name.insert(name.clone(), inputs.clone());
            }
        }
    }

    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        for feature in features {
            if let Some(name) = feature.get("name").and_then(|v| v.as_str())
                && let Some(inputs) = inputs_by_name.get_mut(name)
                && !inputs.is_empty()
            {
                let input = inputs.remove(0);
                feature["inputs"] = serde_json::json!([input]);
            }
        }
    }

    if let Some(feature_data) = value
        .get_mut("feature_data")
        .and_then(|v| v.as_object_mut())
    {
        for (_, data) in feature_data.iter_mut() {
            if let Some(obj) = data.as_object_mut() {
                obj.remove("inputs");
            }
        }
    }
}

/// Move applied flags out of identity/class into a top-level `applied`
/// struct: `identity.species_applied` → `applied.species`,
/// `identity.background_applied` → `applied.background`,
/// `identity.classes[*].applied_levels` → `applied.levels[class_name]`.
fn migrate_v10(value: &mut Value) {
    if value.get("applied").is_some() {
        return;
    }

    let mut species = false;
    let mut background = false;
    let mut levels = serde_json::Map::new();

    if let Some(identity) = value.get_mut("identity").and_then(|v| v.as_object_mut()) {
        if let Some(v) = identity
            .remove("species_applied")
            .or_else(|| identity.remove("race_applied"))
        {
            species = v.as_bool().unwrap_or(false);
        }
        if let Some(v) = identity.remove("background_applied") {
            background = v.as_bool().unwrap_or(false);
        }
        if let Some(classes) = identity.get_mut("classes").and_then(|v| v.as_array_mut()) {
            for class_entry in classes {
                if let Some(obj) = class_entry.as_object_mut() {
                    let class_name = obj
                        .get("class")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    if let Some(applied_levels) = obj.remove("applied_levels")
                        && !class_name.is_empty()
                    {
                        levels.insert(class_name, applied_levels);
                    }
                }
            }
        }
    }

    value["applied"] = serde_json::json!({
        "species": species,
        "background": background,
        "levels": Value::Object(levels),
    });
}

/// Convert weapon `damage` + `damage_type` fields to `effects` array.
fn migrate_v9(value: &mut Value) {
    let Some(weapons) = value
        .pointer_mut("/equipment/weapons")
        .and_then(|v| v.as_array_mut())
    else {
        return;
    };
    for weapon in weapons {
        let damage = weapon
            .get("damage")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let damage_type = weapon.get("damage_type").cloned();

        if !damage.is_empty() {
            let mut effect = serde_json::json!({
                "name": "",
                "expr": damage,
            });
            if let Some(dt) = damage_type {
                effect["damage_type"] = dt;
            }
            weapon["effects"] = serde_json::json!([effect]);
        }

        if let Some(obj) = weapon.as_object_mut() {
            obj.remove("damage");
            obj.remove("damage_type");
        }
    }
}

/// Apply all schema migrations to a raw `Value`, returning the
/// migrated `Value`. Preserves unknown fields that are not part of the current
/// `Character` model — forward-compat safe.
pub fn migrate_value(mut value: Value) -> Value {
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    if version >= CURRENT_SCHEMA_VERSION {
        return value;
    }

    // Migrations are grouped by the schema version they produce. A block
    // `if version < N { … }` runs the steps needed to bring a character up
    // to schema version N. Individual steps stay idempotent — already-
    // migrated shapes are detected and skipped inside each function.

    // → schema version 1: initial batch of data normalizations and
    // structural splits (introduced when the schema_version field was added).
    if version < 1 {
        migrate_v1(&mut value);
        migrate_v2(&mut value);
        migrate_v3(&mut value);
        migrate_v4(&mut value);
        migrate_v5(&mut value);
        migrate_v6(&mut value);
        migrate_v7(&mut value);
        migrate_v8(&mut value);
        migrate_v9(&mut value);
        migrate_v10(&mut value);
        migrate_v11(&mut value);
        migrate_v12(&mut value);
    }

    // → schema version 2: legacy `Weapon.attack_bonus` replaced by
    // `quantity`/`category`/`ability`/`magic_bonus`/`attack_expr`.
    if version < 2 {
        migrate_v13(&mut value);
    }

    // → schema version 3: notes become Vec<Note> with level/date metadata.
    if version < 3 {
        migrate_v14(&mut value);
    }

    // → schema version 4: character name moves from identity to personality;
    // apply-pipeline fields gather under `core`.
    if version < 4 {
        migrate_v15(&mut value);
        migrate_v16(&mut value);
    }

    if let Value::Object(map) = &mut value {
        map.insert("schema_version".into(), Value::from(CURRENT_SCHEMA_VERSION));
    }
    value
}

/// Deserialize a `Value` into a `Character`, applying all
/// migrations. Used for cloud-fetched data.
pub fn deserialize_character_value(value: Value) -> Option<Character> {
    let migrated = migrate_value(value);
    serde_json::from_value(migrated).ok()
}

/// Merge `features: [...]` and `feature_data: {...}` into a single
/// `features: { list: [...], data: {...} }` container.
fn migrate_v12(value: &mut Value) {
    // Already migrated if `features` is an object with `list`.
    if value
        .get("features")
        .is_some_and(|v| v.is_object() && v.get("list").is_some())
    {
        return;
    }

    let list = value
        .get_mut("features")
        .map(|v| v.take())
        .unwrap_or_else(|| serde_json::json!([]));
    let data = value
        .as_object_mut()
        .and_then(|o| o.remove("feature_data"))
        .unwrap_or_else(|| serde_json::json!({}));

    value["features"] = serde_json::json!({
        "list": list,
        "data": data,
    });
}

/// Convert legacy `Weapon.attack_bonus: i32` to the new structured fields:
/// `quantity`, `category`, `ability`, `magic_bonus`, `attack_expr`.
/// The old `attack_bonus` field is reused as `quantity` (many users used
/// that slot for count), clamped to a plausible range.
fn migrate_v13(value: &mut Value) {
    let Some(weapons) = value
        .get_mut("equipment")
        .and_then(|e| e.get_mut("weapons"))
        .and_then(|w| w.as_array_mut())
    else {
        return;
    };

    for weapon in weapons {
        let Some(obj) = weapon.as_object_mut() else {
            continue;
        };

        // Idempotence: skip if already migrated.
        if obj.contains_key("quantity") || obj.contains_key("category") {
            continue;
        }

        let old_bonus = obj
            .remove("attack_bonus")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let quantity: u32 = if (1..=99).contains(&old_bonus) {
            old_bonus as u32
        } else {
            1
        };

        obj.insert("quantity".into(), Value::from(quantity));
        obj.insert("category".into(), Value::from(0u8));
        obj.insert("ability".into(), Value::from(0u8));
        obj.insert("magic_bonus".into(), Value::from(0));
        obj.insert("attack_expr".into(), Value::Null);
    }
}

/// Wrap legacy `notes: String` into a single-entry `Vec<Note>` with
/// `created_at = 0` / `level = 0` markers (unknown metadata for pre-Note data).
/// Empty strings become an empty vector.
fn migrate_v14(value: &mut Value) {
    if value.get("notes").is_some_and(Value::is_array) {
        return;
    }
    let text = value
        .get("notes")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_default();
    value["notes"] = if text.is_empty() {
        serde_json::json!([])
    } else {
        serde_json::json!([{
            "created_at": 0,
            "level": 0,
            "text": text,
        }])
    };
}

/// Lift apply-pipeline fields (identity / abilities / saving_throws /
/// skills / combat / features / proficiencies / languages /
/// damage_modifiers / spell_slots / applied) into a nested `core`
/// object so cascade can clone just the relevant slice. Idempotent:
/// any top-level core-field always wins over the `core` slot — a buggy
/// earlier run of this migration could leave empty defaults inside
/// `core` while the real data lingered at the top level, and that's the
/// only realistic scenario where both sides are populated. Equipment /
/// personality / notes / shared / updated_at / id / schema_version stay
/// at the top level.
fn migrate_v16(value: &mut Value) {
    const CORE_FIELDS: &[&str] = &[
        "identity",
        "abilities",
        "saving_throws",
        "skills",
        "combat",
        "features",
        "proficiencies",
        "languages",
        "damage_modifiers",
        "spell_slots",
        "applied",
    ];
    let Some(map) = value.as_object_mut() else {
        return;
    };
    let mut core = match map.remove("core") {
        Some(Value::Object(map)) => map,
        _ => serde_json::Map::new(),
    };
    for field in CORE_FIELDS {
        if let Some(top_value) = map.remove(*field) {
            core.insert((*field).into(), top_value);
        }
    }
    if !core.is_empty() {
        map.insert("core".into(), Value::Object(core));
    }
}

/// Move `identity.name` → `personality.name` (display-only, not a rules
/// input). Idempotent: skip if `personality.name` already populated.
fn migrate_v15(value: &mut Value) {
    let already_moved = value
        .get("personality")
        .and_then(|p| p.get("name"))
        .is_some_and(|n| n.as_str().is_some_and(|s| !s.is_empty()));
    if already_moved {
        if let Some(identity) = value.get_mut("identity").and_then(Value::as_object_mut) {
            identity.remove("name");
        }
        return;
    }
    let name = value
        .get_mut("identity")
        .and_then(Value::as_object_mut)
        .and_then(|m| m.remove("name"))
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    if name.is_empty() {
        return;
    }
    let personality = value.as_object_mut().map(|m| {
        m.entry("personality")
            .or_insert_with(|| serde_json::json!({}))
    });
    if let Some(p) = personality
        && let Some(obj) = p.as_object_mut()
    {
        obj.insert("name".into(), Value::String(name));
    }
}

/// Move `identity.alignment` → `personality.alignment` (alignment is
/// descriptive, not a rules input).
fn migrate_v11(value: &mut Value) {
    let alignment = value
        .get_mut("identity")
        .and_then(|id| id.as_object_mut())
        .and_then(|id| id.remove("alignment"));

    let Some(alignment) = alignment else {
        return;
    };

    let personality = value.as_object_mut().and_then(|root| {
        root.entry("personality")
            .or_insert_with(|| serde_json::json!({}))
            .as_object_mut()
    });
    if let Some(personality) = personality {
        personality.entry("alignment").or_insert(alignment);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrate_value_preserves_unknown_top_level_fields() {
        let input = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000000",
            "updated_at": 100,
            "abilities": {"str": {"base": 10}},
            "future_unknown_field": {"x": 1, "y": "hi"}
        });
        let migrated = migrate_value(input);
        assert!(
            migrated.get("future_unknown_field").is_some(),
            "unknown field was dropped"
        );
        assert_eq!(
            migrated["schema_version"].as_u64(),
            Some(u64::from(CURRENT_SCHEMA_VERSION))
        );
    }

    #[test]
    fn migrate_value_adds_schema_version_to_legacy_blob() {
        let input = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000000",
            "updated_at": 100,
        });
        let migrated = migrate_value(input);
        assert_eq!(
            migrated["schema_version"].as_u64(),
            Some(u64::from(CURRENT_SCHEMA_VERSION))
        );
    }

    #[test]
    fn migrate_v14_wraps_string_notes_into_single_entry() {
        let mut input = serde_json::json!({"notes": "hello world"});
        migrate_v14(&mut input);
        let notes = input["notes"].as_array().expect("notes must be array");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0]["text"].as_str(), Some("hello world"));
        assert_eq!(notes[0]["created_at"].as_u64(), Some(0));
        assert_eq!(notes[0]["level"].as_u64(), Some(0));
    }

    #[test]
    fn migrate_v14_empty_string_becomes_empty_vec() {
        let mut input = serde_json::json!({"notes": ""});
        migrate_v14(&mut input);
        let notes = input["notes"].as_array().expect("notes must be array");
        assert!(notes.is_empty());
    }

    #[test]
    fn migrate_v14_is_noop_when_already_array() {
        let mut input = serde_json::json!({
            "notes": [{"created_at": 123, "level": 2, "text": "kept"}]
        });
        migrate_v14(&mut input);
        let notes = input["notes"].as_array().expect("notes must be array");
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0]["created_at"].as_u64(), Some(123));
        assert_eq!(notes[0]["level"].as_u64(), Some(2));
        assert_eq!(notes[0]["text"].as_str(), Some("kept"));
    }

    #[test]
    fn migrate_value_skips_when_already_current() {
        let input = serde_json::json!({
            "id": "00000000-0000-0000-0000-000000000000",
            "updated_at": 100,
            "schema_version": CURRENT_SCHEMA_VERSION,
            "marker": "unchanged",
        });
        let migrated = migrate_value(input);
        assert_eq!(migrated["marker"].as_str(), Some("unchanged"));
        assert_eq!(
            migrated["schema_version"].as_u64(),
            Some(u64::from(CURRENT_SCHEMA_VERSION))
        );
    }

    #[test]
    fn migrate_v16_recovers_split_state_from_buggy_prior_run() {
        // Simulates a character whose previous load ran a buggy migrate_v16
        // that created an empty `core` and bumped schema_version while the
        // real data was left at the top level.
        let mut input = serde_json::json!({
            "schema_version": 4,
            "id": "1",
            "core": {
                "identity": {},
                "features": {"data": {}, "list": []},
            },
            "identity": {"species": "Elf", "classes": [{"class": "Wizard", "level": 3}]},
            "features": {"data": {}, "list": [{"name": "Fireball"}]},
            "abilities": {"intelligence": 16},
        });
        migrate_v16(&mut input);
        assert_eq!(
            input["core"]["identity"]["species"].as_str(),
            Some("Elf"),
            "non-empty top-level field must override empty core slot"
        );
        assert_eq!(
            input["core"]["features"]["list"].as_array().map(Vec::len),
            Some(1)
        );
        assert_eq!(
            input["core"]["abilities"]["intelligence"].as_u64(),
            Some(16)
        );
        assert!(input.get("identity").is_none(), "top-level lifted away");
        assert!(input.get("features").is_none());
        assert!(input.get("abilities").is_none());
    }

    #[test]
    fn migrate_v16_top_level_overrides_core() {
        // When both sides have data the top-level wins: the only realistic
        // scenario is the buggy-prior-run case, where `core` carries empty
        // defaults and the real data sits at the top level.
        let mut input = serde_json::json!({
            "core": {"abilities": {"intelligence": 8}},
            "abilities": {"intelligence": 18},
        });
        migrate_v16(&mut input);
        assert_eq!(
            input["core"]["abilities"]["intelligence"].as_u64(),
            Some(18)
        );
        assert!(input.get("abilities").is_none());
    }
}
