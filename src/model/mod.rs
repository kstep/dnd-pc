mod ability;
mod applied;
mod attribute;
mod attribute_group;
mod character;
mod combat;
mod die;
mod effects;
mod enums;
mod equipment;
mod feature;
mod identity;
mod money;
mod skills;
mod spell;

pub use ability::*;
pub use applied::*;
pub use attribute::*;
pub use attribute_group::*;
pub use character::*;
pub use combat::*;
pub use die::*;
pub use effects::*;
pub use enums::*;
pub use equipment::*;
pub use feature::*;
pub use identity::*;
pub use money::*;
pub use skills::*;
pub use spell::*;

/// Expression type with attribute groups for loop support.
pub type Expr = crate::expr::Expr<Attribute, i32, AttributeGroup>;

/// Op type matching [`Expr`].
pub type Op = crate::expr::Op<Attribute, i32, AttributeGroup>;

/// Format an integer as a signed bonus string (e.g. `+3`, `-1`).
pub fn format_bonus(value: i32) -> String {
    if value >= 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}
