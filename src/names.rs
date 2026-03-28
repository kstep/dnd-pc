use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{BASE_URL, rules::utils::fetch_json};

/// Runtime structure: flat species→names lookup built from the JSON's
/// group-based format during deserialization.
#[derive(Clone)]
pub struct NamesData {
    species_map: BTreeMap<Box<str>, NameGroup>,
    default: NameGroup,
}

#[derive(Debug, Clone, Deserialize)]
struct NameGroup {
    first: Vec<Box<str>>,
    last: Vec<Box<str>>,
}

/// JSON shape per group: { species?: [...], first: [...], last: [...] }
#[derive(Deserialize)]
struct RawGroup {
    #[serde(default)]
    species: Vec<Box<str>>,
    first: Vec<Box<str>>,
    last: Vec<Box<str>>,
}

impl NamesData {
    fn from_raw(raw: BTreeMap<Box<str>, RawGroup>) -> Self {
        let mut species_map = BTreeMap::new();
        let mut default = None;

        for (key, group) in raw {
            let names = NameGroup {
                first: group.first,
                last: group.last,
            };
            if &*key == "default" {
                default = Some(names);
            } else {
                for species in group.species {
                    species_map.insert(species, names.clone());
                }
            }
        }

        Self {
            species_map,
            default: default.expect("names.json must have a \"default\" group"),
        }
    }

    pub fn generate_name(&self, species: &str) -> String {
        let group = if species.is_empty() {
            &self.default
        } else {
            self.species_map.get(species).unwrap_or(&self.default)
        };
        let first = random_pick(&group.first);
        let last = random_pick(&group.last);
        format!("{first} {last}")
    }
}

fn random_pick(items: &[Box<str>]) -> &str {
    let index = getrandom::u32().unwrap_or(0) as usize % items.len();
    &items[index]
}

pub async fn fetch_names() -> Option<NamesData> {
    let raw: BTreeMap<Box<str>, RawGroup> = fetch_json(&format!("{BASE_URL}/data/names.json"))
        .await
        .ok()?;
    Some(NamesData::from_raw(raw))
}
