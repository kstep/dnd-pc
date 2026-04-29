use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Write},
    ops::{Deref, Neg},
    str::FromStr,
    sync::Arc,
};

use serde::{Serialize, Serializer, ser::SerializeSeq};

mod de;
mod error;
mod group;
mod interpret;
mod ops;
mod parser;
pub mod stack;
mod tokenizer;
mod traits;

pub use crate::expr::{
    error::Error,
    group::{IterIndex, IterStack, NoGroup, VarGroup, VarSubgroup},
    interpret::{DicePool, ExprAnalysis, Interpreter},
    ops::{BLOCK_ERROR, BLOCK_MAIN, BLOCK_NOOP, BinOp, Block, BlockIndex, Cmp, Op},
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
pub struct Expr<Var, Val = i32, Grp = NoGroup<Var>>(Arc<[Block<Var, Val, Grp>]>);

impl<Var, Val, Grp> Default for Expr<Var, Val, Grp> {
    fn default() -> Self {
        Self(Arc::from([]))
    }
}

impl<Var, Val, Grp> Expr<Var, Val, Grp> {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<Var, Val, Grp> Serialize for Expr<Var, Val, Grp>
where
    Var: Serialize + Copy + PartialEq + fmt::Display,
    Val: Serialize + Copy + fmt::Display,
    Grp: Serialize + Copy + VarGroup<Var = Var> + PartialEq + fmt::Display,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            if self.0.is_empty() {
                return serializer.serialize_str("");
            }
            let s = self
                .format_block(BLOCK_MAIN)
                .map_err(serde::ser::Error::custom)?;
            serializer.serialize_str(&s)
        } else {
            // Binary format: serialize as a sequence of op-blocks.
            let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
            for block in self.0.iter() {
                seq.serialize_element(&block)?;
            }
            seq.end()
        }
    }
}

impl<Var, Val, Grp> Expr<Var, Val, Grp> {
    pub fn block(&self, idx: BlockIndex) -> &Block<Var, Val, Grp> {
        &self.0[idx as usize]
    }

    /// Iterate every block in this expression (main plus sub-blocks).
    pub fn blocks(&self) -> impl Iterator<Item = &Block<Var, Val, Grp>> {
        self.0.iter()
    }

    /// Iterate every op across all blocks, flattened.
    pub fn ops(&self) -> impl Iterator<Item = &Op<Var, Val, Grp>> {
        self.blocks().flat_map(|block| block.iter())
    }

    /// Returns true if any block in this expression contains a variable
    /// matching the predicate.
    pub fn has_var(&self, pred: impl Fn(&Var) -> bool) -> bool
    where
        Grp: VarGroup<Var = Var>,
    {
        self.0.iter().any(|block| block.has_var(&pred))
    }

    /// Returns true if any block contains a dice roll (`Op::Roll`).
    /// Structural check — does not evaluate guards.
    pub fn has_dice(&self) -> bool {
        self.ops().any(|op| matches!(op, Op::Roll))
    }

    /// Smallest `N` such that ARG indices used by this expression are all
    /// `< N`. Covers both direct `Op::PushVar(Arg(n))` references and loop
    /// subgroups whose companion `@ARG(iter_no)` resolves to `Arg(iter_no)`
    /// at run time. `iter_no` is sequential (0, 1, 2, ...) regardless of
    /// mask values — e.g. `@G(3, 5)` binds iterations to `iter_no` 0 and 1,
    /// not 3 and 5. Returns `0` if no ARGs are referenced.
    ///
    /// `arg_index` extracts an ARG's numeric slot from a variable (`Var ->
    /// Option<u8>`); non-ARG variables return `None`.
    pub fn arg_slot_count(&self, arg_index: impl Fn(&Var) -> Option<u8>) -> usize
    where
        Var: Copy,
        Val: Copy,
        Grp: Copy + VarGroup<Var = Var>,
    {
        self.0
            .iter()
            .flat_map(|block| block.iter().copied())
            .filter_map(|op| match op {
                Op::PushVar(var) => arg_index(&var).map(|n| n as usize),
                // Each/Next carry a `VarSubgroup` — masked iteration domain.
                // PushGroup/AssignGroup carry the bare `Grp` (attach to the
                // surrounding Each's iter_stack) and contribute no new
                // iter_no range, so they're not scanned here.
                Op::Each(subgrp) | Op::Next(subgrp) => {
                    subgrp.iter_indices().map(|i| i.iter_no).max()
                }
                _ => None,
            })
            .max()
            .map_or(0, |m| m + 1)
    }

    /// Returns true if any block assigns to a variable matching the predicate.
    pub fn assigns_to(&self, pred: impl Fn(&Var) -> bool) -> bool
    where
        Grp: VarGroup<Var = Var>,
    {
        self.0.iter().any(|block| block.assigns_to(&pred))
    }

    /// Returns true if a specific block or any of its sub-blocks contains a
    /// variable matching the predicate.
    /// Returns true if `idx` refers to a real sub-block (not a sentinel and
    /// within bounds).
    fn is_sub_block(&self, idx: BlockIndex) -> bool {
        idx != BLOCK_NOOP && idx != BLOCK_ERROR && (idx as usize) < self.0.len()
    }

    pub fn block_has_var(&self, block: BlockIndex, pred: &impl Fn(&Var) -> bool) -> bool
    where
        Grp: VarGroup<Var = Var>,
    {
        let mut visited = BTreeSet::new();
        self.block_has_var_inner(block, pred, &mut visited)
    }

    fn block_has_var_inner(
        &self,
        block: BlockIndex,
        pred: &impl Fn(&Var) -> bool,
        visited: &mut BTreeSet<BlockIndex>,
    ) -> bool
    where
        Grp: VarGroup<Var = Var>,
    {
        if !self.is_sub_block(block) || !visited.insert(block) {
            return false;
        }
        let blk = &self.0[block as usize];
        blk.has_var(pred)
            || blk.iter().any(|op| match op {
                Op::Eval(idx) => self.block_has_var_inner(*idx, pred, visited),
                Op::EvalIf(a, b) => {
                    self.block_has_var_inner(*a, pred, visited)
                        || self.block_has_var_inner(*b, pred, visited)
                }
                _ => false,
            })
    }
}

impl<Var, Val, Grp> Expr<Var, Val, Grp> {
    /// Create a new Expr by mapping each op across all blocks.
    pub fn map(&self, mut f: impl FnMut(&Op<Var, Val, Grp>) -> Op<Var, Val, Grp>) -> Self {
        let blocks: Vec<_> = self.0.iter().map(|block| block.map(&mut f)).collect();
        Self(blocks.into())
    }
}

impl<Var, Val, Grp> Deref for Expr<Var, Val, Grp> {
    type Target = Block<Var, Val, Grp>;

    fn deref(&self) -> &Self::Target {
        &self.0[BLOCK_MAIN as usize]
    }
}

impl<Var: Copy, Val: Copy, Grp: Copy> Expr<Var, Val, Grp> {
    pub fn run<I: Interpreter<Var, Val, Grp>>(&self, mut interp: I) -> Result<I::Output, Error> {
        if self.0.is_empty() {
            return Err(Error::EmptyExpression);
        }
        self.run_block(&mut interp, BLOCK_MAIN)?;
        interp.finish()
    }

    fn run_block<I: Interpreter<Var, Val, Grp>>(
        &self,
        interp: &mut I,
        block: BlockIndex,
    ) -> Result<(), Error> {
        let Some(block_ops) = self.0.get(block as usize) else {
            return Err(Error::InvalidBlock(block));
        };
        for &op in block_ops.iter() {
            if let Some(sub_block) = interp.exec(op)? {
                self.run_block(interp, sub_block)?;
            }
        }
        Ok(())
    }
}

impl<Var: Copy + fmt::Display, Grp: Copy + VarGroup<Var = Var>> Expr<Var, i32, Grp> {
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

impl<Var: Copy + fmt::Display, Grp: Copy + VarGroup<Var = Var>> Expr<Var, i32, Grp> {
    pub fn eval_block(
        &self,
        block: BlockIndex,
        ctx: &impl Context<Var, i32>,
    ) -> Result<i32, Error> {
        let mut interp = ReadOnlyEvaluator::new(ctx);
        self.run_block(&mut interp, block)?;
        Interpreter::<Var, i32, Grp>::finish(interp)
    }
}

impl<Var: Copy + fmt::Display, Grp: Copy + VarGroup<Var = Var>> Eval<Var, i32>
    for Expr<Var, i32, Grp>
{
    type Output = Result<i32, Error>;

    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "expr.eval", skip_all)
    )]
    fn eval(&self, ctx: &impl Context<Var, i32>) -> Result<i32, Error> {
        self.run(ReadOnlyEvaluator::new(ctx))
    }

    fn is_dynamic(&self) -> bool {
        true
    }
}

impl<Var: Copy + fmt::Display, Grp: Copy + VarGroup<Var = Var>> Expr<Var, i32, Grp> {
    /// Like `eval`, but silently ignores `Assign` ops instead of erroring.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "expr.eval_lenient", skip_all)
    )]
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
    /// `arg_index` returns `Some(index)` for ARG-like variables (resolved as 0
    /// during analysis), `None` for regular variables.
    ///
    /// Guards with non-interactive false conditions prune their ARGs from
    /// `active_args`.
    #[cfg_attr(
        feature = "perf-marks",
        tracing::instrument(name = "expr.analyze", skip_all)
    )]
    pub fn analyze(
        &self,
        ctx: &impl Context<Var, i32>,
        arg_index: impl Fn(&Var) -> Option<u8> + Copy,
    ) -> ExprAnalysis {
        ExprAnalysis::analyze(self, ctx, arg_index)
    }
}

impl<
    Var: FromStr + Copy + PartialEq,
    Val: FromStr + Copy + Neg<Output = Val>,
    Grp: Default + FromStr + Copy + VarGroup<Var = Var>,
> FromStr for Expr<Var, Val, Grp>
{
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.trim().is_empty() {
            return Ok(Self::default());
        }
        let blocks: Arc<[Block<Var, Val, Grp>]> = Parser::new(s)
            .parse()?
            .into_iter()
            .map(Block::from)
            .collect();
        Ok(Self(blocks))
    }
}

impl<
    Var: Copy + PartialEq + fmt::Display,
    Val: Copy + fmt::Display,
    Grp: Copy + VarGroup<Var = Var> + PartialEq + fmt::Display,
> Expr<Var, Val, Grp>
{
    fn format_block(&self, block: BlockIndex) -> Result<String, Error> {
        let block = &self.0[block as usize];
        let mut results: Vec<String> = Vec::new();
        for stmt in block.statements() {
            if let Some(formatted) = self.try_format_compound(stmt)? {
                results.push(formatted);
            } else {
                results.push(self.format_ops(stmt)?);
            }
        }
        Ok(results.join("; "))
    }

    fn format_ops(&self, ops: &[Op<Var, Val, Grp>]) -> Result<String, Error> {
        let mut fmt = Formatter::new();
        let mut i = 0;
        while i < ops.len() {
            let op = ops[i];
            match op {
                Op::Each(grp) if i + 1 < ops.len() => {
                    if let Op::EvalIf(body_idx, BLOCK_NOOP) = ops[i + 1] {
                        let text = self.format_each_or_fold(grp, body_idx)?;
                        fmt.push_atom(text);
                        i += 2;
                        continue;
                    }
                    fmt.exec(op)?;
                }
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
            i += 1;
        }
        <Formatter as Interpreter<Var, Val, Grp>>::finish(fmt)
    }

    /// Detect each vs fold pattern and format accordingly.
    ///
    /// each: body ends with `[..., Next(grp), EvalIf(body, NOOP)]`
    /// fold: body ends with `[..., Next(grp), EvalIf(acc, NOOP)]`
    ///       where acc = `[Eval(body), BinOp(op)]`
    fn format_each_or_fold(
        &self,
        subgrp: VarSubgroup<Grp>,
        body_idx: BlockIndex,
    ) -> Result<String, Error> {
        let body = &self.0[body_idx as usize];
        let body_ops = &**body;

        // Body must end with Next(grp), EvalIf(_, NOOP)
        let len = body_ops.len();
        if len < 2 {
            return Err(Error::InvalidBlock(body_idx));
        }
        let (Op::EvalIf(target_idx, BLOCK_NOOP), Op::Next(_)) =
            (body_ops[len - 1], body_ops[len - 2])
        else {
            return Err(Error::InvalidBlock(body_idx));
        };

        // The body content is everything before Next + EvalIf
        let content_ops = &body_ops[..len - 2];

        let mut grp_str = String::new();
        write!(grp_str, "{subgrp}").unwrap();

        if target_idx == body_idx {
            let body_text = self.format_block_ops(content_ops)?;
            Ok(format!("each(@{grp_str}, {body_text})"))
        } else {
            let acc_block = &self.0[target_idx as usize];
            let acc_ops = &**acc_block;
            if acc_ops.len() == 2
                && matches!(acc_ops[0], Op::Eval(idx) if idx == body_idx)
                && let Op::BinOp(bin_op) = acc_ops[1]
            {
                // Shorthand: fold(op, @GROUP) when body is just PushGroup(inner)
                if content_ops.len() == 1
                    && matches!(content_ops[0], Op::PushGroup(g) if g == subgrp.inner)
                {
                    Ok(format!("fold({}, @{grp_str})", bin_op.symbol()))
                } else {
                    let expr_text = self.format_block_ops(content_ops)?;
                    Ok(format!(
                        "fold({}, @{grp_str}, {expr_text})",
                        bin_op.symbol()
                    ))
                }
            } else {
                Err(Error::InvalidBlock(body_idx))
            }
        }
    }

    /// Format a slice of ops with compound assignment detection for both
    /// Assign and AssignGroup.
    fn format_block_ops(&self, ops: &[Op<Var, Val, Grp>]) -> Result<String, Error> {
        let mut results: Vec<String> = Vec::new();
        let mut start = 0;
        for (i, op) in ops.iter().enumerate() {
            if matches!(op, Op::AssignVar(_) | Op::AssignGroup(_)) {
                let stmt = &ops[start..=i];
                if let Some(formatted) = self.try_format_compound(stmt)? {
                    results.push(formatted);
                } else {
                    results.push(self.format_ops(stmt)?);
                }
                start = i + 1;
            }
        }
        if start < ops.len() {
            results.push(self.format_ops(&ops[start..])?);
        }
        Ok(results.join("; "))
    }

    /// Try to format a statement as compound assignment (X += Y).
    /// Works for both PushVar/Assign and PushGroup/AssignGroup patterns.
    fn try_format_compound(&self, stmt: &[Op<Var, Val, Grp>]) -> Result<Option<String>, Error> {
        let Some(ca) = Block::detect_compound(stmt) else {
            return Ok(None);
        };
        let var = match stmt.last() {
            Some(Op::AssignVar(v)) => format!("{v}"),
            Some(Op::AssignGroup(g)) => format!("@{g}"),
            _ => unreachable!(),
        };
        let rhs = self.format_ops(&stmt[ca.rhs_start..ca.rhs_end])?;
        Ok(Some(format!("{var} {sym}= {rhs}", sym = ca.sym)))
    }
}

impl<
    Var: Copy + PartialEq + fmt::Display,
    Val: Copy + fmt::Display,
    Grp: Copy + VarGroup<Var = Var> + PartialEq + fmt::Display,
> fmt::Display for Expr<Var, Val, Grp>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if self.0.is_empty() {
            return Ok(());
        }
        let s = self.format_block(BLOCK_MAIN).map_err(|_| fmt::Error)?;
        f.write_str(&s)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

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
        // These use Attribute as the Var type since SLOT.LEVEL/CLASS.LEVEL
        // are not in the local test Var enum.
        use crate::model::Attribute;
        let expr: super::Expr<Attribute> = "(SLOT.LEVEL + 2)d6".parse().unwrap();
        assert_eq!(expr.to_string(), "(SLOT.LEVEL + 2)d6");

        let expr: super::Expr<Attribute> = "(CLASS.LEVEL / 2)d8".parse().unwrap();
        assert_eq!(expr.to_string(), "(CLASS.LEVEL / 2)d8");
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
                Op::BinOp(BinOp::Add),
                Op::PushVar(Var::Modifier(Ability::Dexterity)),
                Op::BinOp(BinOp::Add),
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
                Op::BinOp(BinOp::Add),
                Op::PushVar(Var::Ac),
                Op::PushConst(5),
                Op::BinOp(BinOp::Sub),
                Op::PushVar(Var::Ac),
                Op::PushConst(5),
                Op::BinOp(BinOp::Sub),
                Op::PushConst(2),
                Op::BinOp(BinOp::Mul),
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
        let expr: super::Expr<Attribute> = "(CASTER.MOD + 1)d8!".parse().unwrap();
        assert_eq!(expr.to_string(), "(CASTER.MOD + 1)d8!");
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

    fn arg_index(var: &Var) -> Option<u8> {
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
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(analysis.active_args, BTreeSet::from([0]));

        // guard(AC >= 20, AC += ARG.0) — AC=15 < 20, so ARG.0 is pruned
        let expr: Expr = "guard(AC >= 20, AC += ARG.0)".parse().unwrap();
        let analysis = expr.analyze(&character, arg_index);
        assert!(analysis.active_args.is_empty());

        // Expertise-like pattern: outer if with interactive sum check, inner guards
        // AC=15 → guard(AC>=13) passes, guard(AC>=20) fails, guard(AC>=10) passes
        let expr: Expr =
            "if(ARG.0 + ARG.1 + ARG.2 == 2, guard(AC >= 13, AC += ARG.0); guard(AC >= 20, AC += ARG.1); guard(AC >= 10, AC += ARG.2))"
                .parse()
                .unwrap();
        let analysis = expr.analyze(&character, arg_index);
        // Outer cond ARGs are NOT in active_args (conditions excluded).
        // Only body ARGs from true guards: AC>=13 → ARG.0, AC>=20 false → pruned,
        // AC>=10 → ARG.2.
        assert_eq!(analysis.active_args, BTreeSet::from([0, 2]));

        // Same pattern with if() instead of guard(): non-interactive false
        // conditions still prune. AC=15 → if(AC>=20) false → ARG.1 pruned.
        let expr: Expr =
            "if(ARG.0 + ARG.1 + ARG.2 == 2, if(AC >= 13, AC += ARG.0); if(AC >= 20, AC += ARG.1); if(AC >= 10, AC += ARG.2))"
                .parse()
                .unwrap();
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(analysis.active_args, BTreeSet::from([0, 2]));
    }

    #[wasm_bindgen_test]
    fn analyze_collects_dice() {
        let character = test_character();
        let expr: Expr = "2d6 + 1d8".parse().unwrap();
        let analysis = expr.analyze(&character, arg_index);
        assert_eq!(analysis.dice_rolls.get(&6), Some(&2));
        assert_eq!(analysis.dice_rolls.get(&8), Some(&1));
        assert!(analysis.active_args.is_empty());
    }

    #[wasm_bindgen_test]
    fn analyze_detects_boolean_args() {
        let character = test_character();

        // in(ARG.0, 0, 1) constrains ARG.0 to boolean
        let expr: Expr = "guard(in(ARG.0, 0, 1), STR += ARG.0)".parse().unwrap();
        let analysis = expr.analyze(&character, arg_index);
        assert!(analysis.boolean_args.contains(&0));

        // in(ARG.0, 0, 2) does NOT make ARG.0 boolean
        let expr: Expr = "guard(in(ARG.0, 0, 2), STR += ARG.0)".parse().unwrap();
        let analysis = expr.analyze(&character, arg_index);
        assert!(!analysis.boolean_args.contains(&0));

        // Multiple args, mixed boolean and non-boolean
        let expr: Expr =
            "guard(in(ARG.0, 0, 1) and in(ARG.1, 0, 1) and ARG.0 + ARG.1 == 1, STR += ARG.0; DEX += ARG.1)"
                .parse()
                .unwrap();
        let analysis = expr.analyze(&character, arg_index);
        assert!(analysis.boolean_args.contains(&0));
        assert!(analysis.boolean_args.contains(&1));

        // Non-boolean arg not in boolean_args
        let expr: Expr = "guard(in(ARG.0, 0, 7), STR += ARG.0)".parse().unwrap();
        let analysis = expr.analyze(&character, arg_index);
        assert!(!analysis.boolean_args.contains(&0));
    }

    // --- tier() tests ---

    #[wasm_bindgen_test]
    fn tier_basic_lookup() {
        let character = test_character();
        // STR modifier = 0, DEX modifier = 2, AC = 15
        // Use AC as the variable (15)
        let expr: Expr = "tier(AC, 1:1, 5:2, 11:3, 17:4)".parse().unwrap();
        assert_eq!(expr.eval(&character).unwrap(), 3); // 15 >= 11, < 17

        let expr: Expr = "tier(AC, 1:10, 10:20, 20:30)".parse().unwrap();
        assert_eq!(expr.eval(&character).unwrap(), 20); // 15 >= 10, < 20
    }

    #[wasm_bindgen_test]
    fn tier_exact_threshold() {
        let character = test_character();
        let expr: Expr = "tier(AC, 1:1, 15:2, 20:3)".parse().unwrap();
        assert_eq!(expr.eval(&character).unwrap(), 2); // 15 == 15
    }

    #[wasm_bindgen_test]
    fn tier_below_first_threshold() {
        let character = test_character();
        // STR modifier = 0, threshold starts at 1
        let expr: Expr = "tier(STR, 1:10, 5:20)".parse().unwrap();
        assert_eq!(expr.eval(&character).unwrap(), 0); // 0 < 1
    }

    #[wasm_bindgen_test]
    fn tier_above_all_thresholds() {
        let character = test_character();
        let expr: Expr = "tier(AC, 1:1, 5:2, 10:3)".parse().unwrap();
        assert_eq!(expr.eval(&character).unwrap(), 3); // 15 >= 10
    }

    #[wasm_bindgen_test]
    fn tier_display_roundtrip() {
        let expr: Expr = "tier(AC, 1:1, 5:2, 11:3, 17:4)".parse().unwrap();
        let display = expr.to_string();
        assert_eq!(display, "tier(AC, 1:1, 5:2, 11:3, 17:4)");

        // Round-trip: parse displayed text back
        let reparsed: Expr = display.parse().unwrap();
        assert_eq!(reparsed.to_string(), display);
    }

    #[wasm_bindgen_test]
    fn tier_with_dice() {
        // tier(AC, 1:1, 5:2, 11:3, 17:4)d8 → 3d8
        let expr: Expr = "tier(AC, 1:1, 5:2, 11:3, 17:4)d8".parse().unwrap();
        let display = expr.to_string();
        assert_eq!(display, "tier(AC, 1:1, 5:2, 11:3, 17:4)d8");
    }

    #[wasm_bindgen_test]
    fn tier_compound_assignment() {
        let mut character = test_character();
        // HP (AC=15) += tier(AC, 1:1, 5:2) → AC += 2
        let expr: Expr = "AC += tier(AC, 1:1, 5:2)".parse().unwrap();
        expr.apply(&mut character).unwrap();
        assert_eq!(character.ac, 17); // 15 + 2
    }

    #[wasm_bindgen_test]
    fn tier_empty_error() {
        let result: Result<Expr, _> = "tier(AC)".parse();
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod loop_tests {
    use std::collections::BTreeSet;

    use strum::VariantArray;
    use wasm_bindgen_test::*;

    use crate::{
        expr::{self, Context, Eval, IterIndex, IterStack, Op},
        model::{Ability, Attribute, AttributeGroup, Expr},
    };

    struct TestCtx {
        abilities: [i32; 6],
    }

    impl TestCtx {
        fn new() -> Self {
            Self {
                abilities: [10, 12, 14, 8, 16, 11],
            }
        }
    }

    impl Context<Attribute, i32> for TestCtx {
        fn resolve(&self, var: Attribute) -> Result<i32, expr::Error> {
            match var {
                Attribute::Ability(ability) => {
                    let idx = Ability::VARIANTS
                        .iter()
                        .position(|&a| a == ability)
                        .unwrap();
                    Ok(self.abilities[idx])
                }
                Attribute::Arg(_) => Ok(0),
                _ => Ok(0),
            }
        }

        fn assign(&mut self, var: Attribute, val: i32) -> Result<(), expr::Error> {
            match var {
                Attribute::Ability(ability) => {
                    let idx = Ability::VARIANTS
                        .iter()
                        .position(|&a| a == ability)
                        .unwrap();
                    self.abilities[idx] = val;
                    Ok(())
                }
                _ => Ok(()),
            }
        }
    }

    #[wasm_bindgen_test]
    fn parse_each_roundtrip() {
        let expr: Expr = "each(@ABILITY, @ABILITY += @ARG)".parse().unwrap();
        let display = expr.to_string();
        assert_eq!(display, "each(@ABILITY, @ABILITY += @ARG)");
    }

    #[wasm_bindgen_test]
    fn parse_fold_roundtrip() {
        let expr: Expr = "fold(+, @ABILITY, @ARG)".parse().unwrap();
        let display = expr.to_string();
        assert_eq!(display, "fold(+, @ABILITY, @ARG)");
    }

    #[wasm_bindgen_test]
    fn parse_each_with_condition() {
        let expr: Expr = "each(@ABILITY, if(@ABILITY < 20, @ABILITY += @ARG))"
            .parse()
            .unwrap();
        let display = expr.to_string();
        assert_eq!(
            display,
            "each(@ABILITY, if(@ABILITY < 20, @ABILITY += @ARG))"
        );
    }

    #[wasm_bindgen_test]
    fn parse_fold_with_and() {
        let expr: Expr = "fold(and, @ABILITY, in(@ARG, 0, 1))".parse().unwrap();
        let display = expr.to_string();
        assert_eq!(display, "fold(and, @ABILITY, in(@ARG, 0, 1))");
    }

    #[wasm_bindgen_test]
    fn eval_each_assigns_all() {
        let mut ctx = TestCtx::new();
        let original = ctx.abilities;
        let expr: Expr = "each(@ABILITY, @ABILITY += 1)".parse().unwrap();
        expr.apply(&mut ctx).unwrap();
        for (i, (&original, &actual)) in original.iter().zip(ctx.abilities.iter()).enumerate() {
            assert_eq!(actual, original + 1, "ability {i} not incremented");
        }
    }

    #[wasm_bindgen_test]
    fn eval_fold_sums() {
        let ctx = TestCtx::new();
        // fold(+, @ABILITY, @ABILITY) should sum all ability scores
        let expr: Expr = "fold(+, @ABILITY, @ABILITY)".parse().unwrap();
        let result = expr.eval(&ctx).unwrap();
        let expected: i32 = ctx.abilities.iter().sum();
        assert_eq!(result, expected);
    }

    #[wasm_bindgen_test]
    fn eval_fold_max() {
        let ctx = TestCtx::new();
        let expr: Expr = "fold(max, @ABILITY, @ABILITY)".parse().unwrap();
        let result = expr.eval(&ctx).unwrap();
        assert_eq!(result, *ctx.abilities.iter().max().unwrap());
    }

    #[wasm_bindgen_test]
    fn analyze_each_active_args() {
        let ctx = TestCtx::new();
        let expr: Expr = "each(@ABILITY, @ABILITY += @ARG)".parse().unwrap();
        let analysis = expr.analyze(&ctx, Attribute::arg_index);
        assert_eq!(analysis.active_args.len(), 6);
        assert_eq!(analysis.active_args, BTreeSet::from([0, 1, 2, 3, 4, 5]));
    }

    #[wasm_bindgen_test]
    fn analyze_fold_boolean_args() {
        let ctx = TestCtx::new();
        let expr: Expr = "fold(and, @ABILITY, in(@ARG, 0, 1))".parse().unwrap();
        let analysis = expr.analyze(&ctx, Attribute::arg_index);
        assert_eq!(analysis.active_args.len(), 6);
        for i in 0..6u8 {
            assert!(
                analysis.boolean_args.contains(&i),
                "ARG.{i} should be boolean"
            );
        }
    }

    #[wasm_bindgen_test]
    fn analyze_masked_each_active_args() {
        let ctx = TestCtx::new();
        let expr: Expr = "with(@ABILITY(STR, INT, CHA), each(@, @ += @ARG))"
            .parse()
            .unwrap();
        let analysis = expr.analyze(&ctx, Attribute::arg_index);
        assert_eq!(analysis.active_args.len(), 3);
        // Iteration indices (iter_no), not real group indices
        assert_eq!(analysis.active_args, BTreeSet::from([0, 1, 2]));
    }

    #[wasm_bindgen_test]
    fn analyze_with_guard_masked_fold_each_active_args() {
        // Elemental Affinity expression pattern: with(@GROUP, guard(COND, BODY))
        // where both COND and BODY use @ARG. Regression test: analyze should
        // detect ARGs despite the guard wrapping.
        let ctx = TestCtx::new();
        let expr: Expr = "with(@ABILITY(STR, INT, CHA), guard(fold(and, @, in(@ARG, 0, 1)) and fold(+, @, @ARG) == 1, each(@, @ += @ARG)))"
            .parse()
            .unwrap();
        let analysis = expr.analyze(&ctx, Attribute::arg_index);
        assert!(
            !analysis.active_args.is_empty(),
            "expected non-empty active_args, got {:?}",
            analysis.active_args
        );
    }

    #[wasm_bindgen_test]
    fn analyze_masked_fold_boolean_args() {
        let ctx = TestCtx::new();
        let expr: Expr = "with(@ABILITY(STR, INT, CHA), fold(and, @, in(@ARG, 0, 1)))"
            .parse()
            .unwrap();
        let analysis = expr.analyze(&ctx, Attribute::arg_index);
        assert_eq!(analysis.active_args.len(), 3);
        assert_eq!(analysis.active_args, BTreeSet::from([0, 1, 2]));
        for &i in &[0u8, 1, 2] {
            assert!(
                analysis.boolean_args.contains(&i),
                "ARG.{i} should be boolean"
            );
        }
    }

    #[wasm_bindgen_test]
    fn parse_each_resist() {
        let expr: Expr = "each(@RESIST._, @RESIST._ = 1)".parse().unwrap();
        let display = expr.to_string();
        assert_eq!(display, "each(@RESIST._, @RESIST._ = 1)");
    }

    /// Minimal interpreter that mimics AssignmentSummarizer's behavior:
    /// EvalIf enters then-branch unconditionally (to collect all assignments).
    /// This must NOT infinite-loop on each/fold expressions.
    struct NonEvalInterpreter {
        stack: Vec<Option<i32>>,
        iter_stack: IterStack,
    }

    impl NonEvalInterpreter {
        fn new() -> Self {
            Self {
                stack: Vec::new(),
                iter_stack: IterStack::new(),
            }
        }
    }

    impl expr::Interpreter<Attribute, i32, AttributeGroup> for NonEvalInterpreter {
        type Output = ();

        fn exec(
            &mut self,
            op: Op<Attribute, i32, AttributeGroup>,
        ) -> Result<Option<expr::BlockIndex>, expr::Error> {
            match op {
                Op::PushConst(n) => self.stack.push(Some(n)),
                Op::PushVar(_) => self.stack.push(None),
                Op::PushGroup(_) => self.stack.push(None),
                Op::AssignVar(_) | Op::AssignGroup(_) => {
                    self.stack.pop();
                }
                Op::BinOp(_) | Op::Cmp(_) => {
                    self.stack.pop();
                    self.stack.pop();
                    self.stack.push(None);
                }
                Op::Not => {
                    self.stack.pop();
                    self.stack.push(None);
                }
                Op::In => {
                    self.stack.pop();
                    self.stack.pop();
                    self.stack.pop();
                    self.stack.push(None);
                }
                Op::Each(grp) => {
                    self.iter_stack.push(IterIndex::default());
                    self.stack.push(Some(grp.member(0).is_some() as i32));
                }
                Op::Next(grp) => {
                    if let Ok(entry) = self.iter_stack.top_mut() {
                        entry.iter_no += 1;
                        entry.index += 1;
                        if grp.member(entry.iter_no).is_some() {
                            self.stack.push(Some(1));
                        } else {
                            let _ = self.iter_stack.pop();
                            self.stack.push(Some(0));
                        }
                    } else {
                        self.stack.push(Some(0));
                    }
                }
                // Mimics AssignmentSummarizer: enter then-branch unless
                // condition is known constant(0) (loop termination).
                Op::EvalIf(then_idx, else_idx) => {
                    let cond = self.stack.pop().flatten();
                    if cond != Some(0)
                        && then_idx != expr::BLOCK_NOOP
                        && then_idx != expr::BLOCK_ERROR
                    {
                        return Ok(Some(then_idx));
                    }
                    if else_idx != expr::BLOCK_NOOP && else_idx != expr::BLOCK_ERROR {
                        return Ok(Some(else_idx));
                    }
                }
                Op::Eval(idx) => {
                    if idx != expr::BLOCK_NOOP {
                        return Ok(Some(idx));
                    }
                }
                _ => {
                    self.stack.push(None);
                }
            }
            Ok(None)
        }

        fn finish(self) -> Result<(), expr::Error> {
            Ok(())
        }
    }

    #[wasm_bindgen_test]
    fn summarizer_each_terminates() {
        let expr: Expr = "each(@ABILITY, @ABILITY += @ARG)".parse().unwrap();
        expr.run(NonEvalInterpreter::new()).unwrap();
    }

    #[wasm_bindgen_test]
    fn summarizer_fold_terminates() {
        let expr: Expr = "fold(+, @ABILITY, @ARG)".parse().unwrap();
        expr.run(NonEvalInterpreter::new()).unwrap();
    }

    #[wasm_bindgen_test]
    fn summarizer_guard_with_fold_terminates() {
        let expr: Expr =
            "guard(fold(and, @ABILITY, in(@ARG, 0, 1)) and fold(+, @ABILITY, @ARG) == 1, each(@ABILITY, if(@ABILITY < 20, @ABILITY += @ARG)))"
                .parse()
                .unwrap();
        expr.run(NonEvalInterpreter::new()).unwrap();
    }

    #[wasm_bindgen_test]
    fn summarizer_each_with_condition_terminates() {
        let expr: Expr = "each(@ABILITY, if(@ABILITY < 20, @ABILITY += @ARG))"
            .parse()
            .unwrap();
        expr.run(NonEvalInterpreter::new()).unwrap();
    }

    #[wasm_bindgen_test]
    fn summarizer_fold_with_complex_body_terminates() {
        let expr: Expr = "fold(+, @ABILITY, @ARG + max(0, @ARG - 5))"
            .parse()
            .unwrap();
        expr.run(NonEvalInterpreter::new()).unwrap();
    }

    #[wasm_bindgen_test]
    fn parse_with_binding() {
        let expr: Expr = "with(@ABILITY(INT, WIS, CHA), each(@, if(@ < 20, @ += @ARG)))"
            .parse()
            .unwrap();
        // with expands — display shows the expanded form
        let display = expr.to_string();
        assert!(display.contains("each("));
    }

    #[wasm_bindgen_test]
    fn eval_with_binding() {
        let mut ctx = TestCtx::new();
        // INT=8, WIS=16, CHA=11 → all += 1
        let expr: Expr = "with(@ABILITY(INT, WIS, CHA), each(@, @ += 1))"
            .parse()
            .unwrap();
        expr.apply(&mut ctx).unwrap();
        // STR(0), DEX(1), CON(2) unchanged; INT(3)=9, WIS(4)=17, CHA(5)=12
        assert_eq!(ctx.abilities[0], 10); // STR unchanged
        assert_eq!(ctx.abilities[3], 9); // INT +1
        assert_eq!(ctx.abilities[4], 17); // WIS +1
        assert_eq!(ctx.abilities[5], 12); // CHA +1
    }

    #[wasm_bindgen_test]
    fn eval_with_fold() {
        let ctx = TestCtx::new();
        // fold(+, INT+WIS+CHA) = 8+16+11 = 35
        let expr: Expr = "with(@ABILITY(INT, WIS, CHA), fold(+, @, @))"
            .parse()
            .unwrap();
        let result = expr.eval(&ctx).unwrap();
        assert_eq!(result, 8 + 16 + 11);
    }

    #[wasm_bindgen_test]
    fn summarizer_with_terminates() {
        let expr: Expr =
            "with(@ABILITY(INT, WIS, CHA), guard(fold(and, @, in(@ARG, 0, 1)) and fold(+, @, @ARG) == 1, each(@, if(@ < 20, @ += @ARG))))"
                .parse()
                .unwrap();
        expr.run(NonEvalInterpreter::new()).unwrap();
    }

    #[wasm_bindgen_test]
    fn fold_shorthand_roundtrip() {
        let expr: Expr = "fold(+, @ABILITY)".parse().unwrap();
        assert_eq!(expr.to_string(), "fold(+, @ABILITY)");
    }

    #[wasm_bindgen_test]
    fn eval_fold_shorthand() {
        let ctx = TestCtx::new();
        let expr: Expr = "fold(+, @ABILITY)".parse().unwrap();
        let result = expr.eval(&ctx).unwrap();
        assert_eq!(result, ctx.abilities.iter().sum::<i32>());
    }
}
