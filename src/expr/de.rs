use std::{fmt, marker::PhantomData, ops::Neg, str::FromStr, sync::Arc};

use serde::{Deserialize, Deserializer, de};

use crate::expr::{Block, Expr, Op, VarGroup};

impl<'de, Var, Val, Grp> Deserialize<'de> for Expr<Var, Val, Grp>
where
    Var: FromStr + Copy + PartialEq + Deserialize<'de>,
    Val: FromStr + Copy + Neg<Output = Val> + Deserialize<'de>,
    Grp: Default + FromStr + Copy + VarGroup<Var = Var> + Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ExprVisitor<Var, Val, Grp>(PhantomData<(Var, Val, Grp)>);

        impl<'de, Var, Val, Grp> de::Visitor<'de> for ExprVisitor<Var, Val, Grp>
        where
            Var: FromStr + Copy + PartialEq + Deserialize<'de>,
            Val: FromStr + Copy + Neg<Output = Val> + Deserialize<'de>,
            Grp: Default + FromStr + Copy + VarGroup<Var = Var> + Deserialize<'de>,
        {
            type Value = Expr<Var, Val, Grp>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an expression string or a sequence of ops")
            }

            // Lenient: stale stored exprs (e.g. legacy attribute names from
            // pre-rename data) shouldn't take down the whole character.
            // Log the failure and fall back to an empty expression.
            fn visit_str<E: de::Error>(self, s: &str) -> Result<Expr<Var, Val, Grp>, E> {
                match s.parse() {
                    Ok(expr) => Ok(expr),
                    Err(error) => {
                        log::warn!("Expr parse failed for '{s}': {error}; using default");
                        Ok(Expr::default())
                    }
                }
            }

            fn visit_seq<A: de::SeqAccess<'de>>(
                self,
                seq: A,
            ) -> Result<Expr<Var, Val, Grp>, A::Error> {
                // Try Vec<Vec<Op>> (current format) first, fall back to
                // Vec<Op> (legacy flat format before multi-block expressions).
                #[derive(Deserialize)]
                #[serde(untagged)]
                enum BlocksOrOps<Var, Val, Grp> {
                    Blocks(Vec<Vec<Op<Var, Val, Grp>>>),
                    Ops(Vec<Op<Var, Val, Grp>>),
                }
                let blocks =
                    match BlocksOrOps::deserialize(de::value::SeqAccessDeserializer::new(seq))? {
                        BlocksOrOps::Blocks(blocks) => blocks,
                        BlocksOrOps::Ops(ops) => vec![ops],
                    };
                let blocks: Arc<[Block<Var, Val, Grp>]> =
                    blocks.into_iter().map(Block::from).collect();
                Ok(Expr(blocks))
            }

            fn visit_map<A: de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Expr<Var, Val, Grp>, A::Error> {
                #[derive(serde::Deserialize)]
                struct ExprFields<Var, Val, Grp> {
                    ops: Vec<Vec<Op<Var, Val, Grp>>>,
                }
                let fields = ExprFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                let blocks: Arc<[Block<Var, Val, Grp>]> =
                    fields.ops.into_iter().map(Block::from).collect();
                Ok(Expr(blocks))
            }
        }

        deserializer.deserialize_any(ExprVisitor(PhantomData))
    }
}
