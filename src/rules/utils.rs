use std::{collections::BTreeMap, ops::Deref};

use serde::{Deserialize, Deserializer, de};

use crate::{expr, model::Attribute};

/// A newtype around `BTreeMap<u32, T>` for level-based progressions.
/// Provides `get_for_level()` to find the highest entry at or below a given
/// level, and custom deserialization accepting both numeric and stringified
/// keys.
#[derive(Debug, Clone)]
pub struct LevelRules<T>(BTreeMap<u32, T>);

impl<T> LevelRules<T> {
    pub fn at_level(&self, level: u32) -> Option<&T> {
        self.0.range(..=level).next_back().map(|(_, v)| v)
    }
}

impl<T> LevelRules<T>
where
    T: expr::Eval<Attribute, i32>,
    T::Output: Default,
{
    pub fn eval_for_level(
        &self,
        level: u32,
        ctx: &impl expr::Context<Attribute, i32>,
    ) -> T::Output {
        self.at_level(level)
            .map(|rule| rule.eval(ctx))
            .unwrap_or_default()
    }

    pub fn is_dynamic(&self, level: u32) -> bool {
        self.at_level(level).is_some_and(|rule| rule.is_dynamic())
    }
}

impl<T: Copy + Default> LevelRules<T> {
    pub fn get_for_level(&self, level: u32) -> T {
        self.at_level(level).copied().unwrap_or_default()
    }
}

impl<T> Default for LevelRules<T> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<T> Deref for LevelRules<T> {
    type Target = BTreeMap<u32, T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for LevelRules<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor<T>(std::marker::PhantomData<T>);

        impl<'de, T: Deserialize<'de>> de::Visitor<'de> for Visitor<T> {
            type Value = LevelRules<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map with u32 keys (numeric or stringified)")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: de::MapAccess<'de>,
            {
                let mut result = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<FlexU32, T>()? {
                    result.insert(key.0, value);
                }
                Ok(LevelRules(result))
            }
        }

        deserializer.deserialize_map(Visitor(std::marker::PhantomData))
    }
}

struct FlexU32(u32);

impl<'de> Deserialize<'de> for FlexU32 {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct FlexU32Visitor;

        impl<'de> de::Visitor<'de> for FlexU32Visitor {
            type Value = FlexU32;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("u32 or stringified u32")
            }

            fn visit_u32<E: de::Error>(self, v: u32) -> Result<FlexU32, E> {
                Ok(FlexU32(v))
            }

            fn visit_u64<E: de::Error>(self, v: u64) -> Result<FlexU32, E> {
                u32::try_from(v).map(FlexU32).map_err(de::Error::custom)
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<FlexU32, E> {
                v.parse().map(FlexU32).map_err(de::Error::custom)
            }
        }

        deserializer.deserialize_any(FlexU32Visitor)
    }
}

pub async fn fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    do_fetch_json(url).await.inspect_err(|error| {
        log::error!("fetch_json {url} failed: {error}");
    })
}

async fn do_fetch_json<T: for<'de> Deserialize<'de>>(url: &str) -> Result<T, String> {
    let resp = gloo_net::http::Request::get(url)
        .send()
        .await
        .map_err(|error| format!("fetch error: {error}"))?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    resp.json()
        .await
        .map_err(|error| format!("parse error: {error}"))
}
