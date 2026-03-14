use std::{collections::BTreeMap, fmt, marker::PhantomData, ops::Deref, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de};

mod error;
mod interpret;
mod parser;
mod stack;
mod tokenizer;

pub use crate::expr::{
    error::Error,
    interpret::{DicePool, Interpreter},
};
use crate::expr::{
    interpret::{DicePoolEvaluator, Evaluator, Formatter, ReadOnlyEvaluator},
    parser::Parser,
};

pub trait Context<Var> {
    fn assign(&mut self, var: Var, value: i32) -> Result<(), Error>;
    fn resolve(&self, var: Var) -> Result<i32, Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Expr<Var>(Vec<Op<Var>>);

impl<Var> Deref for Expr<Var> {
    type Target = [Op<Var>];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<Var: Copy> Expr<Var> {
    pub fn run<I: Interpreter<Var>>(&self, interp: I) -> Result<I::Output, Error> {
        interp.run(self.iter().copied())
    }
}

impl<Var: Copy + fmt::Display> Expr<Var> {
    pub fn apply(&self, ctx: &mut impl Context<Var>) -> Result<i32, Error> {
        self.run(Evaluator::new(ctx))
    }

    pub fn apply_with_dice(
        &self,
        ctx: &mut impl Context<Var>,
        pool: &DicePool,
    ) -> Result<i32, Error> {
        let mut iter = pool.iter();
        self.run(DicePoolEvaluator::new(ctx, &mut iter))
    }

    pub fn eval(&self, ctx: &impl Context<Var>) -> Result<i32, Error> {
        self.run(ReadOnlyEvaluator::new(ctx))
    }
}

impl<Var: Copy> Expr<Var> {
    /// Scans the ops for `[PushConst(count), PushConst(sides), Roll]` patterns
    /// and returns a map of die sides to total number of rolls needed.
    pub fn dice_rolls(&self) -> BTreeMap<u32, u32> {
        let mut result = BTreeMap::new();
        for window in self.0.windows(3) {
            if let [Op::PushConst(count), Op::PushConst(sides), Op::Roll] = window
                && *count > 0
                && *sides > 0
            {
                *result.entry(*sides as u32).or_insert(0) += *count as u32;
            }
        }
        result
    }
}

impl<Var: FromStr + Copy> FromStr for Expr<Var> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Parser::new(s).parse().map(Self)
    }
}

impl<'de, Var: FromStr + Copy + Deserialize<'de>> Deserialize<'de> for Expr<Var> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ExprVisitor<Var>(PhantomData<Var>);

        impl<'de, Var: FromStr + Copy + Deserialize<'de>> de::Visitor<'de> for ExprVisitor<Var> {
            type Value = Expr<Var>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an expression string or a sequence of ops")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Expr<Var>, E> {
                s.parse().map_err(de::Error::custom)
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Expr<Var>, A::Error> {
                let ops = Vec::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                Ok(Expr(ops))
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Expr<Var>, A::Error> {
                #[derive(serde::Deserialize)]
                struct ExprFields<Var> {
                    ops: Vec<Op<Var>>,
                }
                let fields = ExprFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(Expr(fields.ops))
            }
        }

        deserializer.deserialize_any(ExprVisitor(PhantomData))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op<Var> {
    PushVar(Var),
    PushConst(i32),
    Add,      // +
    Sub,      // -
    Mul,      // *
    DivFloor, // /
    DivCeil,  // \
    Min,
    Max,
    Roll,         // 2d20 -> 2 20 Roll Sum
    KeepMax(u32), // 2d20kh1 -> 2 20 Roll KeepMax(1)
    KeepMin(u32),
    DropMax(u32),
    DropMin(u32),
    Sum,
    Assign(Var),
    Mod, // %
}

impl<Var: Copy + fmt::Display> fmt::Display for Expr<Var> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = self.run(Formatter::new()).map_err(|_| fmt::Error)?;
        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use wasm_bindgen_test::*;

    use super::*;
    use crate::model::{Ability, AbilityScores};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    enum Var {
        Modifier(Ability),
        Ac,
    }

    impl fmt::Display for Var {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Var::Modifier(Ability::Strength) => write!(f, "STR"),
                Var::Modifier(Ability::Dexterity) => write!(f, "DEX"),
                Var::Modifier(Ability::Constitution) => write!(f, "CON"),
                Var::Modifier(Ability::Intelligence) => write!(f, "INT"),
                Var::Modifier(Ability::Wisdom) => write!(f, "WIS"),
                Var::Modifier(Ability::Charisma) => write!(f, "CHA"),
                Var::Ac => write!(f, "AC"),
            }
        }
    }

    impl FromStr for Var {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "STR" => Ok(Var::Modifier(Ability::Strength)),
                "DEX" => Ok(Var::Modifier(Ability::Dexterity)),
                "CON" => Ok(Var::Modifier(Ability::Constitution)),
                "INT" => Ok(Var::Modifier(Ability::Intelligence)),
                "WIS" => Ok(Var::Modifier(Ability::Wisdom)),
                "CHA" => Ok(Var::Modifier(Ability::Charisma)),
                "AC" => Ok(Var::Ac),
                _ => Err(()),
            }
        }
    }

    type Expr = super::Expr<Var>;

    struct Character {
        #[allow(dead_code)]
        abilities: AbilityScores,
        ac: i32,
    }

    fn test_character() -> Character {
        Character {
            abilities: AbilityScores {
                strength: 10,
                dexterity: 14,
                constitution: 12,
                intelligence: 9,
                wisdom: 13,
                charisma: 18,
            },
            ac: 15,
        }
    }

    impl Context<Var> for Character {
        fn assign(&mut self, var: Var, value: i32) -> Result<(), Error> {
            match var {
                Var::Ac => {
                    self.ac = value;
                    Ok(())
                }
                _ => unimplemented!(),
            }
        }

        fn resolve(&self, var: Var) -> Result<i32, Error> {
            match var {
                Var::Modifier(Ability::Strength) => Ok(0),
                Var::Modifier(Ability::Dexterity) => Ok(2),
                Var::Modifier(Ability::Constitution) => Ok(1),
                Var::Modifier(Ability::Intelligence) => Ok(-1),
                Var::Modifier(Ability::Wisdom) => Ok(1),
                Var::Modifier(Ability::Charisma) => Ok(4),
                Var::Ac => Ok(self.ac),
            }
        }
    }

    #[wasm_bindgen_test]
    fn display_expr() {
        let expr: Expr = "10 + CHA + DEX".parse().unwrap();
        assert_eq!(expr.to_string(), "10 + CHA + DEX");

        let expr: Expr = "2 * (3 + 4)".parse().unwrap();
        assert_eq!(expr.to_string(), "2 * (3 + 4)");

        let expr: Expr = "2d6kh1 + 3".parse().unwrap();
        assert_eq!(expr.to_string(), "2d6kh1 + 3");

        let expr: Expr = "AC + 5; AC - 5; (AC - 5) * 2".parse().unwrap();
        assert_eq!(expr.to_string(), "AC + 5; AC - 5; (AC - 5) * 2");
    }

    #[wasm_bindgen_test]
    fn sorcery_resilience() {
        let ch = test_character();

        // 10 + CHA + DEX
        let expr: Expr = "10 + CHA + DEX".parse().unwrap();
        assert_eq!(
            expr.0,
            vec![
                Op::PushConst(10),
                Op::PushVar(Var::Modifier(Ability::Charisma)),
                Op::Add,
                Op::PushVar(Var::Modifier(Ability::Dexterity)),
                Op::Add,
            ]
        );

        let value = expr.eval(&ch).unwrap();
        assert_eq!(value, 16);
    }

    #[wasm_bindgen_test]
    fn expr_sequence() {
        let ch = test_character();

        let expr: Expr = "AC + 5; AC - 5; (AC - 5) * 2".parse().unwrap();
        assert_eq!(
            expr.0,
            vec![
                Op::PushVar(Var::Ac),
                Op::PushConst(5),
                Op::Add,
                Op::PushVar(Var::Ac),
                Op::PushConst(5),
                Op::Sub,
                Op::PushVar(Var::Ac),
                Op::PushConst(5),
                Op::Sub,
                Op::PushConst(2),
                Op::Mul,
            ]
        );

        let value = expr.eval(&ch).unwrap();
        assert_eq!(value, 20);
    }

    #[wasm_bindgen_test]
    fn arithmetic() {
        let ch = test_character();

        let expr: Expr = "2 * 3 + 4".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 10);

        let expr: Expr = "2 + 3 * 4".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 14);

        let expr: Expr = "(2 + 3) * 4".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 20);
    }

    #[wasm_bindgen_test]
    fn unary_minus() {
        let ch = test_character();
        let expr: Expr = "-5 + 10".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 5);
    }

    #[wasm_bindgen_test]
    fn min_max() {
        let ch = test_character();

        let expr: Expr = "min(3, 7)".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 3);

        let expr: Expr = "max(3, 7)".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 7);
    }

    #[wasm_bindgen_test]
    fn dice_parse() {
        let expr: Expr = "2d6".parse().unwrap();
        assert_eq!(
            expr.0,
            vec![Op::PushConst(2), Op::PushConst(6), Op::Roll, Op::Sum]
        );

        let expr: Expr = "4d6kh3".parse().unwrap();
        assert_eq!(
            expr.0,
            vec![Op::PushConst(4), Op::PushConst(6), Op::Roll, Op::KeepMax(3)]
        );
    }

    #[wasm_bindgen_test]
    fn ability_modifiers() {
        let ch = test_character();
        // STR 10 -> mod 0, DEX 14 -> mod 2, CON 12 -> mod 1
        // INT 9 -> mod -1, WIS 13 -> mod 1, CHA 18 -> mod 4
        assert_eq!("STR".parse::<Expr>().unwrap().eval(&ch).unwrap(), 0);
        assert_eq!("DEX".parse::<Expr>().unwrap().eval(&ch).unwrap(), 2);
        assert_eq!("CON".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("INT".parse::<Expr>().unwrap().eval(&ch).unwrap(), -1);
        assert_eq!("WIS".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("CHA".parse::<Expr>().unwrap().eval(&ch).unwrap(), 4);
    }

    #[wasm_bindgen_test]
    fn modulo() {
        let ch = test_character();

        let expr: Expr = "10 % 3".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 1);

        let expr: Expr = "7 % 2 + 1".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 2);

        // Precedence: % binds like * and /
        let expr: Expr = "2 + 10 % 3".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 3);

        let expr: Expr = "10 % 3".parse().unwrap();
        assert_eq!(expr.to_string(), "10 % 3");
    }

    #[wasm_bindgen_test]
    fn dice_rolls_analysis() {
        let expr: Expr = "2d6 + 1d20".parse().unwrap();
        let rolls = expr.dice_rolls();
        assert_eq!(rolls[&6], 2);
        assert_eq!(rolls[&20], 1);

        // Multiple dice of same type are summed
        let expr: Expr = "2d6 + 3d6".parse().unwrap();
        let rolls = expr.dice_rolls();
        assert_eq!(rolls[&6], 5);

        // No dice
        let expr: Expr = "10 + AC".parse().unwrap();
        let rolls = expr.dice_rolls();
        assert!(rolls.is_empty());
    }

    #[wasm_bindgen_test]
    fn dice_pool_evaluator() {
        let mut ch = test_character();
        let expr: Expr = "2d6 + 3".parse().unwrap();

        let pool: DicePool = BTreeMap::from([(6, vec![3, 5])]).into();
        let result = expr.apply_with_dice(&mut ch, &pool).unwrap();
        assert_eq!(result, 3 + 5 + 3); // 11
    }

    #[wasm_bindgen_test]
    fn dice_pool_keep_highest() {
        let mut ch = test_character();
        let expr: Expr = "4d6kh3".parse().unwrap();

        let pool: DicePool = BTreeMap::from([(6, vec![2, 5, 1, 4])]).into();
        let result = expr.apply_with_dice(&mut ch, &pool).unwrap();
        // Keep highest 3 of [2, 5, 1, 4] = 5 + 4 + 2 = 11
        assert_eq!(result, 11);
    }

    #[wasm_bindgen_test]
    fn dice_pool_exhausted() {
        let mut ch = test_character();
        let expr: Expr = "3d6".parse().unwrap();

        // Only 2 values for d6, but need 3
        let pool: DicePool = BTreeMap::from([(6, vec![3, 5])]).into();
        let result = expr.apply_with_dice(&mut ch, &pool);
        assert_eq!(result, Err(Error::DicePoolExhausted(6)));
    }

    #[wasm_bindgen_test]
    fn dice_pool_mixed_dice() {
        let mut ch = test_character();
        let expr: Expr = "1d20 + 2d6".parse().unwrap();

        let pool: DicePool = BTreeMap::from([(20, vec![15]), (6, vec![3, 4])]).into();
        let result = expr.apply_with_dice(&mut ch, &pool).unwrap();
        assert_eq!(result, 15 + 3 + 4); // 22
    }

    #[wasm_bindgen_test]
    fn compound_assignment() {
        let mut ch = test_character();

        // AC starts at 15
        let expr: Expr = "AC += 5".parse().unwrap();
        assert_eq!(expr.apply(&mut ch).unwrap(), 20);
        assert_eq!(ch.ac, 20);

        // Desugars to same ops as expanded form
        let compound: Expr = "AC -= 3".parse().unwrap();
        let expanded: Expr = "AC = AC - 3".parse().unwrap();
        assert_eq!(compound.0, expanded.0);

        // All compound operators
        ch.ac = 10;
        let expr: Expr = "AC *= 2".parse().unwrap();
        assert_eq!(expr.apply(&mut ch).unwrap(), 20);

        ch.ac = 20;
        let expr: Expr = "AC /= 3".parse().unwrap();
        assert_eq!(expr.apply(&mut ch).unwrap(), 6);

        ch.ac = 20;
        let expr: Expr = "AC \\= 3".parse().unwrap();
        assert_eq!(expr.apply(&mut ch).unwrap(), 7);

        ch.ac = 17;
        let expr: Expr = "AC %= 5".parse().unwrap();
        assert_eq!(expr.apply(&mut ch).unwrap(), 2);

        // Display: compound shows as compound
        let expr: Expr = "AC += 5".parse().unwrap();
        assert_eq!(expr.to_string(), "AC += 5");

        let expr: Expr = "AC -= 3".parse().unwrap();
        assert_eq!(expr.to_string(), "AC -= 3");

        let expr: Expr = "AC *= 2".parse().unwrap();
        assert_eq!(expr.to_string(), "AC *= 2");

        let expr: Expr = "AC /= 3".parse().unwrap();
        assert_eq!(expr.to_string(), "AC /= 3");

        let expr: Expr = "AC \\= 3".parse().unwrap();
        assert_eq!(expr.to_string(), "AC \\= 3");

        let expr: Expr = "AC %= 5".parse().unwrap();
        assert_eq!(expr.to_string(), "AC %= 5");

        // Non-compound: different var on left
        let expr: Expr = "AC = DEX + 10".parse().unwrap();
        assert_eq!(expr.to_string(), "AC = DEX + 10");

        // Non-compound: complex left side
        let expr: Expr = "AC = AC * 2 + 1".parse().unwrap();
        assert_eq!(expr.to_string(), "AC = AC * 2 + 1");
    }
}
