mod ability;
mod attribute;
mod character;
mod combat;
mod die;
mod effects;
mod enums;
mod equipment;
mod feature;
mod identity;
mod money;
mod spell;

pub use ability::*;
pub use attribute::*;
pub use character::*;
pub use combat::*;
pub use die::*;
pub use effects::*;
pub use enums::*;
pub use equipment::*;
pub use feature::*;
pub use identity::*;
pub use money::*;
pub use spell::*;

/// Format an integer as a signed bonus string (e.g. `+3`, `-1`).
pub fn format_bonus(value: i32) -> String {
    if value >= 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}
