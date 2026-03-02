mod character;
mod enums;

pub use character::*;
pub use enums::*;

/// Format an integer as a signed bonus string (e.g. `+3`, `-1`).
pub fn format_bonus(value: i32) -> String {
    if value >= 0 {
        format!("+{value}")
    } else {
        value.to_string()
    }
}
