use std::collections::{BTreeMap, btree_map};

use serde::{Deserialize, Serialize};

use crate::model::{ProficiencyLevel, Skill};

/// Map of skill → proficiency level. Absent entries = `ProficiencyLevel::None`.
/// Setters auto-prune entries that would carry the `None` level so the map
/// stays minimal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Skills(
    #[serde(deserialize_with = "crate::serde_util::deserialize_map_dropping_nulls")]
    BTreeMap<Skill, ProficiencyLevel>,
);

impl Skills {
    pub fn get(&self, skill: Skill) -> ProficiencyLevel {
        self.0
            .get(&skill)
            .copied()
            .unwrap_or(ProficiencyLevel::None)
    }

    pub fn set(&mut self, skill: Skill, level: ProficiencyLevel) {
        if level == ProficiencyLevel::None {
            self.0.remove(&skill);
        } else {
            self.0.insert(skill, level);
        }
    }

    /// Cycle: None → Proficient → Expertise → None.
    pub fn cycle(&mut self, skill: Skill) {
        self.set(skill, self.get(skill).next());
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }

    pub fn iter(&self) -> btree_map::Iter<'_, Skill, ProficiencyLevel> {
        self.0.iter()
    }
}

impl FromIterator<(Skill, ProficiencyLevel)> for Skills {
    fn from_iter<I: IntoIterator<Item = (Skill, ProficiencyLevel)>>(iter: I) -> Self {
        let mut skills = Skills::default();
        for (skill, level) in iter {
            skills.set(skill, level);
        }
        skills
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn deserialize_skips_null_entries_and_preserves_valid_ones() {
        // Null values arrive via the Firestore merge-tombstone path. Dropping
        // them on read keeps Character loadable instead of failing with
        // "Invalid character file".
        let value = json!({
            "0": 1,
            "10": null,
            "11": 2,
            "14": null,
        });
        let skills: Skills = serde_json::from_value(value).expect("must deserialize");
        assert_eq!(skills.get(Skill::Acrobatics), ProficiencyLevel::Proficient);
        assert_eq!(skills.get(Skill::Nature), ProficiencyLevel::None);
        assert_eq!(skills.get(Skill::Perception), ProficiencyLevel::Expertise);
        assert_eq!(skills.get(Skill::Religion), ProficiencyLevel::None);
    }

    #[test]
    fn deserialize_empty_map_ok() {
        let _: Skills = serde_json::from_value(json!({})).expect("empty must deserialize");
    }
}
