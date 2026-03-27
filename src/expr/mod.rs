use std::{
    collections::BTreeMap,
    fmt,
    ops::{Deref, Neg},
    str::FromStr,
    sync::Arc,
};

use serde::{Serialize, Serializer, ser::SerializeSeq};

mod de;
mod error;
mod interpret;
mod ops;
mod parser;
mod stack;
mod tokenizer;
mod traits;

pub use crate::expr::{
    error::Error,
    interpret::{DicePool, ExprAnalysis, Interpreter},
    ops::{BLOCK_ERROR, BLOCK_MAIN, BLOCK_NOOP, Block, BlockIndex, Op},
    traits::{Context, Eval},
};
use crate::expr::{
    interpret::{DicePoolEvaluator, Evaluator, Formatter, ReadOnlyEvaluator},
    parser::Parser,
};

/// Average hit points per level for a given hit die: `sides / 2 + 1`.
pub const fn avg_hp(sides: i32) -> i32 {
    sides / 2 + 1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr<Var, Val = i32>(Arc<[Block<Var, Val>]>);

impl<Var, Val> Serialize for Expr<Var, Val>
where
    Var: Serialize + Copy + PartialEq + fmt::Display,
    Val: Serialize + Copy + fmt::Display,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            // JSON/Firestore: serialize as infix string (avoids nested arrays
            // which Firestore rejects).
            let s = self
                .format_block(BLOCK_MAIN)
                .map_err(serde::ser::Error::custom)?;
            serializer.serialize_str(&s)
        } else {
            // postcard (binary): serialize as ops for compact sharing URLs.
            let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
            for block in self.0.iter() {
                seq.serialize_element(&block)?;
            }
            seq.end()
        }
    }
}

impl<Var, Val> Expr<Var, Val> {
    pub fn block(&self, idx: BlockIndex) -> &Block<Var, Val> {
        &self.0[idx as usize]
    }

    /// Returns true if any block in this expression contains a variable
    /// matching the predicate.
    pub fn has_var(&self, pred: impl Fn(&Var) -> bool) -> bool {
        self.0.iter().any(|block| block.has_var(&pred))
    }

    /// Returns true if a specific block or any of its sub-blocks contains a
    /// variable matching the predicate.
    /// Returns true if `idx` refers to a real sub-block (not a sentinel and
    /// within bounds).
    fn is_sub_block(&self, idx: BlockIndex) -> bool {
        idx != BLOCK_NOOP && idx != BLOCK_ERROR && (idx as usize) < self.0.len()
    }

    pub fn block_has_var(&self, block: BlockIndex, pred: &impl Fn(&Var) -> bool) -> bool {
        if !self.is_sub_block(block) {
            return false;
        }
        let blk = &self.0[block as usize];
        blk.has_var(pred)
            || blk.iter().any(|op| match op {
                Op::Eval(idx) => self.block_has_var(*idx, pred),
                Op::EvalIf(a, b) => self.block_has_var(*a, pred) || self.block_has_var(*b, pred),
                _ => false,
            })
    }
}

impl<Var, Val> Expr<Var, Val> {
    /// Replace ops across all blocks by applying a mapping function.
    /// Creates a new Expr with the transformed blocks.
    pub fn partial_apply(&self, mut f: impl FnMut(&Op<Var, Val>) -> Op<Var, Val>) -> Self {
        let blocks: Vec<_> = self
            .0
            .iter()
            .map(|block| {
                let ops: Vec<_> = block.iter().map(&mut f).collect();
                Block::from(ops)
            })
            .collect();
        Self(blocks.into())
    }
}

impl<Var, Val> Deref for Expr<Var, Val> {
    type Target = Block<Var, Val>;

    fn deref(&self) -> &Self::Target {
        &self.0[BLOCK_MAIN as usize]
    }
}

impl<Var: Copy, Val: Copy> Expr<Var, Val> {
    pub fn run<I: Interpreter<Var, Val>>(&self, mut interp: I) -> Result<I::Output, Error> {
        self.run_block(&mut interp, BLOCK_MAIN)?;
        interp.finish()
    }

    fn run_block<I: Interpreter<Var, Val>>(
        &self,
        interp: &mut I,
        block: BlockIndex,
    ) -> Result<(), Error> {
        for &op in self.0[block as usize].iter() {
            if let Some(sub_block) = interp.exec(op)? {
                self.run_block(interp, sub_block)?;
            }
        }
        Ok(())
    }
}

impl<Var: Copy + fmt::Display> Expr<Var, i32> {
    pub fn apply(&self, ctx: &mut impl Context<Var, i32>) -> Result<i32, Error> {
        self.run(Evaluator::new(ctx))
    }

    pub fn apply_with_dice(
        &self,
        ctx: &mut impl Context<Var, i32>,
        pool: &DicePool,
    ) -> Result<i32, Error> {
        let mut iter = pool.iter();
        self.run(DicePoolEvaluator::new(ctx, &mut iter))
    }
}

impl<Var: Copy + fmt::Display> Expr<Var, i32> {
    pub fn eval_block(
        &self,
        block: BlockIndex,
        ctx: &impl Context<Var, i32>,
    ) -> Result<i32, Error> {
        let mut interp = ReadOnlyEvaluator::new(ctx);
        self.run_block(&mut interp, block)?;
        interp.finish()
    }
}

impl<Var: Copy + fmt::Display> Eval<Var, i32> for Expr<Var, i32> {
    type Output = Result<i32, Error>;

    fn eval(&self, ctx: &impl Context<Var, i32>) -> Result<i32, Error> {
        self.run(ReadOnlyEvaluator::new(ctx))
    }

    fn is_dynamic(&self) -> bool {
        true
    }
}

impl<Var: Copy + fmt::Display> Expr<Var, i32> {
    /// Like `eval`, but silently ignores `Assign` ops instead of erroring.
    pub fn eval_lenient(&self, ctx: &impl Context<Var, i32>) -> Result<i32, Error> {
        self.run(ReadOnlyEvaluator::lenient(ctx))
    }

    /// Evaluates the expression against the context to determine dice roll
    /// requirements. Returns a map of die sides to total number of rolls
    /// needed. Supports both static (`2d6`) and dynamic (`(LEVEL / 5 + 1)d6`)
    /// dice counts.
    pub fn dice_rolls(&self, ctx: &impl Context<Var, i32>) -> BTreeMap<u32, u32> {
        self.analyze(ctx, |_| None).dice_rolls
    }

    /// Analyze the expression: collect dice requirements and determine which
    /// ARG variables are reachable.
    ///
    /// `is_arg` returns `Some(index)` for ARG-like variables (resolved as 0
    /// during analysis), `None` for regular variables.
    ///
    /// Guards with non-interactive false conditions prune their ARGs from
    /// `active_args`.
    pub fn analyze(
        &self,
        ctx: &impl Context<Var, i32>,
        is_arg: impl Fn(&Var) -> Option<u8> + Copy,
    ) -> ExprAnalysis {
        ExprAnalysis::analyze(self, ctx, is_arg)
    }
}

impl<Var: FromStr + Copy, Val: FromStr + Copy + Neg<Output = Val>> FromStr for Expr<Var, Val> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let blocks: Arc<[Block<Var, Val>]> = Parser::new(s)
            .parse()?
            .into_iter()
            .map(Block::from)
            .collect();
        Ok(Self(blocks))
    }
}

impl<Var: Copy + PartialEq + fmt::Display, Val: Copy + fmt::Display> Expr<Var, Val> {
    fn format_block(&self, block: BlockIndex) -> Result<String, Error> {
        let block = &self.0[block as usize];
        let mut results: Vec<String> = Vec::new();
        for stmt in block.statements() {
            if let Some(ca) = Block::detect_compound(stmt) {
                let var = match stmt.last() {
                    Some(Op::Assign(v)) => v,
                    _ => unreachable!(),
                };
                let rhs = self.format_ops(&stmt[ca.rhs_start..ca.rhs_end])?;
                results.push(format!("{var} {sym}= {rhs}", sym = ca.sym));
            } else {
                let text = self.format_ops(stmt)?;
                results.push(text);
            }
        }
        Ok(results.join("; "))
    }

    fn format_ops(&self, ops: &[Op<Var, Val>]) -> Result<String, Error> {
        let mut fmt = Formatter::new();
        for &op in ops {
            match op {
                Op::Eval(idx) => {
                    let text = self.format_block(idx)?;
                    fmt.push_atom(text);
                }
                Op::EvalIf(then_idx, BLOCK_ERROR) => {
                    let cond = fmt.pop_text()?;
                    let then_text = self.format_block(then_idx)?;
                    fmt.push_atom(format!("guard({cond}, {then_text})"));
                }
                Op::EvalIf(then_idx, else_idx) => {
                    let cond = fmt.pop_text()?;
                    let then_text = self.format_block(then_idx)?;
                    if else_idx == BLOCK_NOOP {
                        fmt.push_atom(format!("if({cond}, {then_text})"));
                    } else {
                        let else_text = self.format_block(else_idx)?;
                        fmt.push_atom(format!("if({cond}, {then_text}, {else_text})"));
                    }
                }
                op => {
                    fmt.exec(op)?;
                }
            }
        }
        <Formatter as Interpreter<Var, Val>>::finish(fmt)
    }
}

impl<Var: Copy + PartialEq + fmt::Display, Val: Copy + fmt::Display> fmt::Display
    for Expr<Var, Val>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let s = self.format_block(BLOCK_MAIN).map_err(|_| fmt::Error)?;
        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde::Deserialize;
    use wasm_bindgen_test::*;

    use super::*;
    use crate::model::{Ability, AbilityScores};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    enum Var {
        Modifier(Ability),
        Ac,
        Arg(u8),
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
                Var::Arg(n) => write!(f, "ARG.{n}"),
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
                _ if s.starts_with("ARG.") => s[4..].parse::<u8>().map(Var::Arg).map_err(|_| ()),
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

    impl Context<Var, i32> for Character {
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
                Var::Arg(_) => Ok(0),
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

        // Dice with complex amount/sides must keep parentheses
        // These use Attribute as the Var type since SLOT_LEVEL/CLASS_LEVEL
        // are not in the local test Var enum.
        use crate::model::Attribute;
        let expr: super::Expr<Attribute> = "(SLOT_LEVEL + 2)d6".parse().unwrap();
        assert_eq!(expr.to_string(), "(SLOT_LEVEL + 2)d6");

        let expr: super::Expr<Attribute> = "(CLASS_LEVEL / 2)d8".parse().unwrap();
        assert_eq!(expr.to_string(), "(CLASS_LEVEL / 2)d8");
    }

    #[wasm_bindgen_test]
    fn sorcery_resilience() {
        let ch = test_character();

        // 10 + CHA + DEX
        let expr: Expr = "10 + CHA + DEX".parse().unwrap();
        assert_eq!(
            &**expr,
            &[
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
            &**expr,
            &[
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
            &**expr,
            &[Op::PushConst(2), Op::PushConst(6), Op::Roll, Op::Sum]
        );

        let expr: Expr = "4d6kh3".parse().unwrap();
        assert_eq!(
            &**expr,
            &[Op::PushConst(4), Op::PushConst(6), Op::Roll, Op::KeepMax(3)]
        );
    }

    #[wasm_bindgen_test]
    fn dice_in_function_call() {
        let expr: Expr = "max(AC, 1d4 + 4)".parse().unwrap();
        // Should parse without error — dice inside function args
        assert!(!expr.is_empty());
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
        let ch = test_character();

        let expr: Expr = "2d6 + 1d20".parse().unwrap();
        let rolls = expr.dice_rolls(&ch);
        assert_eq!(rolls[&6], 2);
        assert_eq!(rolls[&20], 1);

        // Multiple dice of same type are summed
        let expr: Expr = "2d6 + 3d6".parse().unwrap();
        let rolls = expr.dice_rolls(&ch);
        assert_eq!(rolls[&6], 5);

        // No dice
        let expr: Expr = "10 + AC".parse().unwrap();
        let rolls = expr.dice_rolls(&ch);
        assert!(rolls.is_empty());

        // Dynamic dice count: AC is 15, so (AC - 13)d8 = 2d8
        let expr: Expr = "(AC - 13)d8".parse().unwrap();
        let rolls = expr.dice_rolls(&ch);
        assert_eq!(rolls[&8], 2);

        // Assignment with dice: AC += 2d6
        let expr: Expr = "AC += 2d6".parse().unwrap();
        let rolls = expr.dice_rolls(&ch);
        assert_eq!(rolls[&6], 2);

        // Dynamic count evaluating to zero
        let expr: Expr = "(AC - 15)d6".parse().unwrap();
        let rolls = expr.dice_rolls(&ch);
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
    fn exploding_dice_parse() {
        let expr: Expr = "3d8!".parse().unwrap();
        assert_eq!(
            &**expr,
            &[Op::PushConst(3), Op::PushConst(8), Op::Roll, Op::Explode]
        );
    }

    #[wasm_bindgen_test]
    fn exploding_dice_display() {
        let expr: Expr = "3d8!".parse().unwrap();
        assert_eq!(expr.to_string(), "3d8!");

        let expr: Expr = "d20!".parse().unwrap();
        assert_eq!(expr.to_string(), "d20!");

        use crate::model::Attribute;
        let expr: super::Expr<Attribute> = "(CASTER_MODIFIER + 1)d8!".parse().unwrap();
        assert_eq!(expr.to_string(), "(CASTER_MODIFIER + 1)d8!");
    }

    #[wasm_bindgen_test]
    fn exploding_dice_pool() {
        let mut ch = test_character();
        let expr: Expr = "3d8!".parse().unwrap();

        // All rolls are max (8) — sum all 3
        let pool: DicePool = BTreeMap::from([(8, vec![8, 8, 8])]).into();
        let result = expr.apply_with_dice(&mut ch, &pool).unwrap();
        assert_eq!(result, 24);

        // First roll is not max — only sum first die
        let pool: DicePool = BTreeMap::from([(8, vec![5, 8, 8])]).into();
        let result = expr.apply_with_dice(&mut ch, &pool).unwrap();
        assert_eq!(result, 5);

        // First two are max, third is not — sum all 3
        let pool: DicePool = BTreeMap::from([(8, vec![8, 8, 3])]).into();
        let result = expr.apply_with_dice(&mut ch, &pool).unwrap();
        assert_eq!(result, 19);
    }

    #[wasm_bindgen_test]
    fn exploding_dice_rolls_analysis() {
        let ch = test_character();

        // Exploding dice should report full pool
        let expr: Expr = "3d8!".parse().unwrap();
        let rolls = expr.dice_rolls(&ch);
        assert_eq!(rolls[&8], 3);
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
        assert_eq!(*compound, *expanded);

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

        // Compound with chained additions
        let expr: Expr = "AC += DEX + 10".parse().unwrap();
        assert_eq!(expr.to_string(), "AC += DEX + 10");

        // Compound with complex rhs
        let expr: Expr = "AC += INT + DEX - 2".parse().unwrap();
        assert_eq!(expr.to_string(), "AC += INT + DEX - 2");

        // Multi-statement compound
        let expr: Expr = "AC += INT; AC -= 2".parse().unwrap();
        assert_eq!(expr.to_string(), "AC += INT; AC -= 2");

        // Compound subtraction with multi-term rhs (no redundant parens)
        let expr: Expr = "AC -= INT + 5".parse().unwrap();
        assert_eq!(expr.to_string(), "AC -= INT + 5");

        // Compound with sub-expression that needs internal parens
        let expr: Expr = "AC -= 3 - 1".parse().unwrap();
        assert_eq!(expr.to_string(), "AC -= 3 - 1");

        // Subtraction does not propagate (x - a + b ≠ x - (a + b))
        let expr: Expr = "AC = AC - DEX + 2".parse().unwrap();
        assert_eq!(expr.to_string(), "AC = AC - DEX + 2");
    }

    #[wasm_bindgen_test]
    fn comparison_ops() {
        let ch = test_character();

        assert_eq!("3 > 2".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("2 > 3".parse::<Expr>().unwrap().eval(&ch).unwrap(), 0);
        assert_eq!("3 >= 3".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("3 < 4".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("3 <= 3".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("1 == 1".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("1 == 2".parse::<Expr>().unwrap().eval(&ch).unwrap(), 0);
        assert_eq!("1 != 2".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("1 != 1".parse::<Expr>().unwrap().eval(&ch).unwrap(), 0);

        // With expressions
        assert_eq!("AC >= 13".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1); // AC=15
        assert_eq!("AC + 1 > 15".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
    }

    #[wasm_bindgen_test]
    fn boolean_ops() {
        let ch = test_character();

        assert_eq!(
            "1 > 0 and 2 > 1"
                .parse::<Expr>()
                .unwrap()
                .eval(&ch)
                .unwrap(),
            1
        );
        assert_eq!(
            "1 > 0 and 0 > 1"
                .parse::<Expr>()
                .unwrap()
                .eval(&ch)
                .unwrap(),
            0
        );
        assert_eq!(
            "1 > 0 or 0 > 1".parse::<Expr>().unwrap().eval(&ch).unwrap(),
            1
        );
        assert_eq!(
            "0 > 1 or 0 > 2".parse::<Expr>().unwrap().eval(&ch).unwrap(),
            0
        );
        assert_eq!("not 0".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("not 1".parse::<Expr>().unwrap().eval(&ch).unwrap(), 0);

        // Precedence: and binds tighter than or
        assert_eq!(
            "0 or 1 and 1".parse::<Expr>().unwrap().eval(&ch).unwrap(),
            1
        ); // 0 or (1 and 1) = 1
        assert_eq!(
            "1 or 1 and 0".parse::<Expr>().unwrap().eval(&ch).unwrap(),
            1
        ); // 1 or (1 and 0) = 1

        // Parenthesized
        assert_eq!(
            "(AC >= 13) and (CHA >= 3)"
                .parse::<Expr>()
                .unwrap()
                .eval(&ch)
                .unwrap(),
            1
        );
    }

    #[wasm_bindgen_test]
    fn if_function() {
        let ch = test_character();

        assert_eq!(
            "if(1, 10, 20)".parse::<Expr>().unwrap().eval(&ch).unwrap(),
            10
        );
        assert_eq!(
            "if(0, 10, 20)".parse::<Expr>().unwrap().eval(&ch).unwrap(),
            20
        );
        assert_eq!(
            "if(AC > 10, AC, 10)"
                .parse::<Expr>()
                .unwrap()
                .eval(&ch)
                .unwrap(),
            15
        );
    }

    #[wasm_bindgen_test]
    fn display_boolean() {
        assert_eq!(
            "3 >= 2 and 1 < 5".parse::<Expr>().unwrap().to_string(),
            "3 >= 2 and 1 < 5"
        );
        assert_eq!(
            "1 > 0 or 2 > 0".parse::<Expr>().unwrap().to_string(),
            "1 > 0 or 2 > 0"
        );
        assert_eq!(
            "not (AC > 3)".parse::<Expr>().unwrap().to_string(),
            "not (AC > 3)"
        );
        assert_eq!(
            "if(AC > 0, AC, 0)".parse::<Expr>().unwrap().to_string(),
            "if(AC > 0, AC, 0)"
        );
        // Precedence in display: or groups and
        assert_eq!(
            "(1 or 2) and 3".parse::<Expr>().unwrap().to_string(),
            "(1 or 2) and 3"
        );
    }

    #[wasm_bindgen_test]
    fn average_dice() {
        let expr: Expr = "avg_hp(6)".parse().unwrap();
        assert_eq!(expr.to_string(), "avg_hp(6)");

        let ch = test_character();
        assert_eq!(expr.eval(&ch).unwrap(), 4);

        for (sides, expected) in [(4, 3), (6, 4), (8, 5), (10, 6), (12, 7), (20, 11)] {
            let expr: Expr = format!("avg_hp({sides})").parse().unwrap();
            assert_eq!(
                expr.eval(&ch).unwrap(),
                expected,
                "avg_hp({sides}) should be {expected}"
            );
        }
    }

    #[wasm_bindgen_test]
    fn guard_syntax() {
        let mut ch = test_character();

        // Display round-trip
        let expr: Expr = "guard(AC >= 13, AC += 2)".parse().unwrap();
        assert_eq!(expr.to_string(), "guard(AC >= 13, AC += 2)");

        // Guard passes: AC=15 >= 13 → AC += 2
        ch.ac = 15;
        assert_eq!(expr.apply(&mut ch).unwrap(), 17);
        assert_eq!(ch.ac, 17);

        // Guard fails: AC=10 < 13 → error
        ch.ac = 10;
        assert_eq!(expr.apply(&mut ch), Err(Error::GuardFailed));

        // Guard inside if
        let expr: Expr = "if(1, guard(AC >= 13, AC += 1))".parse().unwrap();
        assert_eq!(expr.to_string(), "if(1, guard(AC >= 13, AC += 1))");
    }

    fn is_arg(var: &Var) -> Option<u8> {
        match var {
            Var::Arg(n) => Some(*n),
            _ => None,
        }
    }

    #[wasm_bindgen_test]
    fn analyze_guard_prunes_args() {
        let character = test_character(); // AC=15

        // guard(AC >= 13, AC += ARG.0) — AC=15 >= 13, so ARG.0 is active
        let expr: Expr = "guard(AC >= 13, AC += ARG.0)".parse().unwrap();
        let analysis = expr.analyze(&character, is_arg);
        assert_eq!(analysis.active_args, vec![0]);

        // guard(AC >= 20, AC += ARG.0) — AC=15 < 20, so ARG.0 is pruned
        let expr: Expr = "guard(AC >= 20, AC += ARG.0)".parse().unwrap();
        let analysis = expr.analyze(&character, is_arg);
        assert!(analysis.active_args.is_empty());

        // Expertise-like pattern: outer if with interactive sum check, inner guards
        // AC=15 → guard(AC>=13) passes, guard(AC>=20) fails, guard(AC>=10) passes
        let expr: Expr =
            "if(ARG.0 + ARG.1 + ARG.2 == 2, guard(AC >= 13, AC += ARG.0); guard(AC >= 20, AC += ARG.1); guard(AC >= 10, AC += ARG.2))"
                .parse()
                .unwrap();
        let analysis = expr.analyze(&character, is_arg);
        // Outer cond ARGs are NOT in active_args (conditions excluded).
        // Only body ARGs from true guards: AC>=13 → ARG.0, AC>=20 false → pruned,
        // AC>=10 → ARG.2.
        let mut active = analysis.active_args.clone();
        active.sort();
        assert_eq!(active, vec![0, 2]);

        // Same pattern with if() instead of guard(): non-interactive false
        // conditions still prune. AC=15 → if(AC>=20) false → ARG.1 pruned.
        let expr: Expr =
            "if(ARG.0 + ARG.1 + ARG.2 == 2, if(AC >= 13, AC += ARG.0); if(AC >= 20, AC += ARG.1); if(AC >= 10, AC += ARG.2))"
                .parse()
                .unwrap();
        let analysis = expr.analyze(&character, is_arg);
        let mut active = analysis.active_args.clone();
        active.sort();
        assert_eq!(active, vec![0, 2]);
    }

    #[wasm_bindgen_test]
    fn analyze_collects_dice() {
        let character = test_character();
        let expr: Expr = "2d6 + 1d8".parse().unwrap();
        let analysis = expr.analyze(&character, is_arg);
        assert_eq!(analysis.dice_rolls.get(&6), Some(&2));
        assert_eq!(analysis.dice_rolls.get(&8), Some(&1));
        assert!(analysis.active_args.is_empty());
    }
}
