use std::collections::{BTreeMap, btree_map};

use serde::{Deserialize, Serialize};

use crate::model::{ProficiencyLevel, Skill};

/// Map of skill → proficiency level. Absent entries = `ProficiencyLevel::None`.
/// Setters auto-prune entries that would carry the `None` level so the map
/// stays minimal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Skills(BTreeMap<Skill, ProficiencyLevel>);

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
