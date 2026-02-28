use std::io::{Read, Write};

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::model::Character;

fn strip_for_sharing(character: &Character) -> Character {
    let mut character = character.clone();

    character.combat.death_save_successes = 0;
    character.combat.death_save_failures = 0;
    character.combat.hp_temp = 0;

    for feature in &mut character.features {
        feature.description.clear();
    }

    for racial_trait in &mut character.racial_traits {
        racial_trait.description.clear();
    }

    for entry in character.feature_data.values_mut() {
        for field in &mut entry.fields {
            field.description.clear();
            for opt in field.value.choices_mut() {
                opt.description.clear();
            }
        }
        if let Some(spells) = &mut entry.spells {
            for spell in &mut spells.spells {
                spell.description.clear();
            }
        }
    }

    character
}

pub fn encode_character(character: &Character) -> String {
    let character = strip_for_sharing(character);
    let bytes = postcard::to_allocvec(&character).expect("failed to serialize character");
    let mut compressed = Vec::new();
    {
        let mut encoder = brotli::CompressorWriter::new(&mut compressed, 4096, 11, 22);
        encoder.write_all(&bytes).expect("failed to compress");
    }
    let encoded = URL_SAFE_NO_PAD.encode(&compressed);

    log::info!(
        "share character: bytes={}, compressed={}, encoded={}, value={encoded}",
        bytes.len(),
        compressed.len(),
        encoded.len()
    );

    encoded
}

pub fn decode_character(data: &str) -> Option<Character> {
    let compressed = URL_SAFE_NO_PAD.decode(data).ok()?;
    let mut decoder = brotli::Decompressor::new(&compressed[..], 4096);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).ok()?;
    let ch: Character = postcard::from_bytes(&bytes).ok()?;
    Some(ch)
}

#[cfg(test)]
pub mod tests {
    use std::collections::{BTreeMap, HashMap, HashSet};

    use uuid::Uuid;
    use wasm_bindgen_test::*;

    use super::*;
    use crate::{
        model::{
            Ability, AbilityScores, Alignment, CharacterIdentity, ClassLevel, CombatStats,
            Equipment, Feature, FeatureData, Personality, Spell, SpellData,
        },
        vecset::VecSet,
    };

    fn test_character() -> Character {
        let mut ch = Character {
            id: Uuid::nil(),
            identity: CharacterIdentity {
                name: "Share Test".to_string(),
                classes: vec![ClassLevel {
                    class: "Bard".to_string(),
                    subclass: None,
                    level: 3,
                    hit_die_sides: 8,
                    hit_dice_used: 0,
                    applied_levels: VecSet::new(),
                    caster_coef: 1,
                }],
                race: "Elf".to_string(),
                background: "Entertainer".to_string(),
                alignment: Alignment::ChaoticGood,
                experience_points: 900,
                race_applied: true,
                background_applied: true,
            },
            abilities: AbilityScores {
                strength: 8,
                dexterity: 14,
                constitution: 12,
                intelligence: 10,
                wisdom: 13,
                charisma: 16,
            },
            saving_throws: HashSet::from([Ability::Dexterity, Ability::Charisma]),
            skills: HashMap::new(),
            combat: CombatStats {
                armor_class: 13,
                speed: 30,
                hp_max: 24,
                hp_current: 20,
                hp_temp: 5,
                death_save_successes: 2,
                death_save_failures: 1,
                initiative_misc_bonus: 0,
            },
            personality: Personality::default(),
            features: vec![Feature {
                name: "Bardic Inspiration".to_string(),
                description: "Use a bonus action...".to_string(),
            }],
            equipment: Equipment::default(),
            feature_data: BTreeMap::from([(
                "Spellcasting (Bard)".to_string(),
                FeatureData {
                    fields: Vec::new(),
                    spells: Some(SpellData {
                        casting_ability: Ability::Charisma,
                        spells: vec![Spell {
                            name: "Vicious Mockery".to_string(),
                            level: 0,
                            prepared: true,
                            description: "Unleash a string of insults...".to_string(),
                            sticky: false,
                        }],
                    }),
                },
            )]),
            proficiencies: HashSet::new(),
            languages: VecSet::new(),
            racial_traits: Vec::new(),
            spell_slots: Vec::new(),
            notes: String::new(),
            updated_at: 0,
        };
        ch.update_spell_slots(None);
        ch
    }

    #[wasm_bindgen_test]
    fn encode_decode_roundtrip() {
        let ch = test_character();
        let encoded = encode_character(&ch);
        let decoded = decode_character(&encoded).expect("decode failed");

        // Core identity preserved
        assert_eq!(decoded.id, ch.id);
        assert_eq!(decoded.identity.name, "Share Test");
        assert_eq!(decoded.identity.classes[0].class, "Bard");
        assert_eq!(decoded.identity.classes[0].level, 3);
        assert_eq!(decoded.abilities.charisma, 16);

        // Stripped fields should be zeroed
        assert_eq!(decoded.combat.death_save_successes, 0);
        assert_eq!(decoded.combat.death_save_failures, 0);
        assert_eq!(decoded.combat.hp_temp, 0);

        // Descriptions should be cleared
        assert!(decoded.features[0].description.is_empty());
    }

    #[wasm_bindgen_test]
    fn strip_zeros_death_saves_and_hp_temp() {
        let ch = test_character();
        let stripped = strip_for_sharing(&ch);

        assert_eq!(stripped.combat.death_save_successes, 0);
        assert_eq!(stripped.combat.death_save_failures, 0);
        assert_eq!(stripped.combat.hp_temp, 0);

        // hp_current and hp_max should be preserved
        assert_eq!(stripped.combat.hp_current, 20);
        assert_eq!(stripped.combat.hp_max, 24);
    }

    #[wasm_bindgen_test]
    fn strip_clears_descriptions() {
        let ch = test_character();
        let stripped = strip_for_sharing(&ch);

        // Feature descriptions cleared
        assert!(stripped.features[0].description.is_empty());

        // Spell descriptions cleared
        let spell_data = stripped
            .feature_data
            .get("Spellcasting (Bard)")
            .unwrap()
            .spells
            .as_ref()
            .unwrap();
        assert!(spell_data.spells[0].description.is_empty());

        // But spell name preserved
        assert_eq!(spell_data.spells[0].name, "Vicious Mockery");
    }

    #[wasm_bindgen_test]
    fn decode_garbage_returns_none() {
        assert!(decode_character("not-valid-data!!!").is_none());
        assert!(decode_character("").is_none());
        assert!(decode_character("AAAA").is_none());
    }
}
