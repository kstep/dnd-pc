use strum::Display;

/// Borrowed reference to one of the four reference-browser entry kinds
/// (class / species / background / spell list). Routing helper: `prefix()` +
/// `name()` build `/r/<prefix>/<name>` hrefs; `RulesRegistry::entry_label_desc`
/// dispatches on it for locale-aware labels.
#[derive(Copy, Clone, Display)]
pub enum IndexEntry<'a> {
    #[strum(to_string = "class.{0}")]
    Class(&'a str),
    #[strum(to_string = "species.{0}")]
    Species(&'a str),
    #[strum(to_string = "background.{0}")]
    Background(&'a str),
    #[strum(to_string = "spell.{0}")]
    Spell(&'a str),
}

impl<'a> IndexEntry<'a> {
    pub fn name(&self) -> &'a str {
        match *self {
            Self::Class(n) | Self::Species(n) | Self::Background(n) | Self::Spell(n) => n,
        }
    }

    /// Path prefix (`"class"` / `"species"` / …) for `/r/<prefix>/<name>`
    /// routes.
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Class(_) => "class",
            Self::Species(_) => "species",
            Self::Background(_) => "background",
            Self::Spell(_) => "spell",
        }
    }
}
