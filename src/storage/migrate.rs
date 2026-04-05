use std::collections::HashMap;

use crate::model::{Character, DamageType};

/// Migrate legacy string damage_type values to u8 enum representation.
fn migrate_v1(value: &mut serde_json::Value) {
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
                    Some(d) => serde_json::Value::Number((d as u8).into()),
                    None => serde_json::Value::Null,
                };
                weapon["damage_type"] = new_val;
            }
        }
    }
}

/// Migrate flat spell_slots array to BTreeMap keyed by pool.
fn migrate_v2(value: &mut serde_json::Value) {
    if value
        .get("spell_slots")
        .is_some_and(|slots| slots.is_array())
    {
        let slots = value["spell_slots"].take();
        value["spell_slots"] = serde_json::json!({ "0": slots });
    }
}

/// Migrate string attack_bonus to i32.
fn migrate_v3(value: &mut serde_json::Value) {
    if let Some(weapons) = value
        .get_mut("equipment")
        .and_then(|e| e.get_mut("weapons"))
        .and_then(|w| w.as_array_mut())
    {
        for weapon in weapons {
            if let Some(s) = weapon.get("attack_bonus").and_then(|v| v.as_str()) {
                let parsed: i32 = s.parse().unwrap_or(0);
                weapon["attack_bonus"] = serde_json::Value::Number(parsed.into());
            }
        }
    }
}

/// Migrate FeatureValue::Die from string to { die, used } struct.
fn migrate_v4(value: &mut serde_json::Value) {
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
fn migrate_v5(value: &mut serde_json::Value) {
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

    let langs: Vec<serde_json::Value> = correct_langs
        .iter()
        .map(|l| serde_json::Value::String(l.to_string()))
        .collect();
    value["languages"] = serde_json::Value::Array(langs);
}

/// Set `applied: true` on all features that lack the field (old characters).
fn migrate_v6(value: &mut serde_json::Value) {
    if let Some(features) = value.get_mut("features").and_then(|v| v.as_array_mut()) {
        for feature in features {
            if feature.get("applied").is_none() {
                feature["applied"] = serde_json::Value::Bool(true);
            }
        }
    }
}

/// Migrate Feature entries: add `source` from FeatureData.source, remove
/// FeatureData.source. Convert FeatureSource::Class(String) to
/// Class(String, u32).
fn migrate_v7(value: &mut serde_json::Value) {
    let mut sources: HashMap<String, serde_json::Value> = HashMap::new();
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
            if (feature.get("source").is_none()
                || feature.get("source") == Some(&serde_json::Value::Null))
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
                        let migrated: Vec<serde_json::Value> = arr
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
                        serde_json::Value::Array(migrated)
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
fn migrate_v8(value: &mut serde_json::Value) {
    let mut inputs_by_name: HashMap<String, Vec<serde_json::Value>> = HashMap::new();
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

/// Convert weapon `damage` + `damage_type` fields to `effects` array.
fn migrate_v9(value: &mut serde_json::Value) {
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

/// Deserialize a `serde_json::Value` into a `Character`, applying all
/// migrations. Used for cloud-fetched data.
pub fn deserialize_character_value(mut value: serde_json::Value) -> Option<Character> {
    migrate_v1(&mut value);
    migrate_v2(&mut value);
    migrate_v3(&mut value);
    migrate_v4(&mut value);
    migrate_v5(&mut value);
    migrate_v6(&mut value);
    migrate_v7(&mut value);
    migrate_v8(&mut value);
    migrate_v9(&mut value);
    serde_json::from_value(value).ok()
}
