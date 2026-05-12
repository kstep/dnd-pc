use serde::{Deserialize, Serialize};

use crate::model::{Ability, ProficiencyLevel};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Tools(Vec<ToolEntry>);

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolEntry {
    pub name: String,
    pub prof: ProficiencyLevel,
    #[serde(default)]
    pub ability: Ability,
}

impl Tools {
    pub fn level(&self, name: &str) -> ProficiencyLevel {
        self.0
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.prof)
            .unwrap_or(ProficiencyLevel::None)
    }

    pub fn ability(&self, name: &str) -> Option<Ability> {
        self.0
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.ability)
    }

    pub fn set(&mut self, name: &str, prof: ProficiencyLevel) {
        if prof == ProficiencyLevel::None {
            if let Some(idx) = self.0.iter().position(|entry| entry.name == name) {
                self.0.remove(idx);
            }
        } else if let Some(entry) = self.0.iter_mut().find(|entry| entry.name == name) {
            entry.prof = prof;
        } else {
            self.0.push(ToolEntry {
                name: name.to_string(),
                prof,
                ability: Ability::Strength,
            });
        }
    }

    pub fn as_mut(&mut self, name: &str) -> Option<&mut ToolEntry> {
        self.0.iter_mut().find(|entry| entry.name == name)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn set_count(&mut self, n: usize) {
        if n > self.0.len() {
            self.0.resize_with(n, || ToolEntry {
                name: String::new(),
                prof: ProficiencyLevel::Proficient,
                ability: Ability::Strength,
            });
        } else {
            self.0.truncate(n);
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ToolEntry> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, ToolEntry> {
        self.0.iter_mut()
    }

    pub fn push_empty(&mut self) {
        self.0.push(ToolEntry {
            name: String::new(),
            prof: ProficiencyLevel::Proficient,
            ability: Ability::Strength,
        });
    }

    pub fn remove(&mut self, idx: usize) {
        if idx < self.0.len() {
            self.0.remove(idx);
        }
    }

    pub fn get_mut(&mut self, idx: usize) -> Option<&mut ToolEntry> {
        self.0.get_mut(idx)
    }

    pub fn clear(&mut self) {
        self.0.clear();
    }
}

impl FromIterator<ToolEntry> for Tools {
    fn from_iter<I: IntoIterator<Item = ToolEntry>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_creates_then_updates_then_removes() {
        let mut tools = Tools::default();
        tools.set("Smith's Tools", ProficiencyLevel::Proficient);
        assert_eq!(tools.level("Smith's Tools"), ProficiencyLevel::Proficient);
        assert_eq!(tools.len(), 1);

        tools.set("Smith's Tools", ProficiencyLevel::Expertise);
        assert_eq!(tools.level("Smith's Tools"), ProficiencyLevel::Expertise);
        assert_eq!(tools.len(), 1);

        tools.set("Smith's Tools", ProficiencyLevel::None);
        assert_eq!(tools.level("Smith's Tools"), ProficiencyLevel::None);
        assert_eq!(tools.len(), 0);
    }

    #[test]
    fn set_count_grows_with_proficient_placeholders_and_truncates() {
        let mut tools = Tools::default();
        tools.set_count(3);
        assert_eq!(tools.len(), 3);
        for entry in tools.iter() {
            assert!(entry.name.is_empty());
            assert_eq!(entry.prof, ProficiencyLevel::Proficient);
        }
        tools.set_count(1);
        assert_eq!(tools.len(), 1);
    }

    #[test]
    fn level_returns_none_for_absent() {
        let tools = Tools::default();
        assert_eq!(tools.level("Anything"), ProficiencyLevel::None);
    }

    #[test]
    fn deserialize_empty_default() {
        let tools: Tools = serde_json::from_str("[]").unwrap();
        assert!(tools.is_empty());
    }

    #[test]
    fn round_trip_serialization() {
        let mut tools = Tools::default();
        tools.set("Lute", ProficiencyLevel::Proficient);
        tools.set("Thieves' Tools", ProficiencyLevel::Expertise);
        let json = serde_json::to_string(&tools).unwrap();
        let parsed: Tools = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, tools);
    }

    #[test]
    fn serialize_emits_object_array_not_nested_tuples() {
        let mut tools = Tools::default();
        tools.set("Lute", ProficiencyLevel::Proficient);
        let json = serde_json::to_value(&tools).unwrap();
        assert_eq!(
            json,
            serde_json::json!([{"name": "Lute", "prof": 1, "ability": 0}])
        );
    }
}
