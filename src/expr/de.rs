use std::{fmt, marker::PhantomData, ops::Neg, str::FromStr, sync::Arc};

use serde::{Deserialize, Deserializer, de};

use crate::expr::{Expr, Op};

impl<'de, Var, Val> Deserialize<'de> for Expr<Var, Val>
where
    Var: FromStr + Copy + Deserialize<'de>,
    Val: FromStr + Copy + Neg<Output = Val> + Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ExprVisitor<Var, Val>(PhantomData<(Var, Val)>);

        impl<'de, Var, Val> de::Visitor<'de> for ExprVisitor<Var, Val>
        where
            Var: FromStr + Copy + Deserialize<'de>,
            Val: FromStr + Copy + Neg<Output = Val> + Deserialize<'de>,
        {
            type Value = Expr<Var, Val>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an expression string or a sequence of ops")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Expr<Var, Val>, E> {
                s.parse().map_err(de::Error::custom)
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Expr<Var, Val>, A::Error> {
                let blocks: Vec<Vec<Op<Var, Val>>> =
                    Vec::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                #[allow(clippy::type_complexity)]
                let blocks: Arc<[Box<[Op<Var, Val>]>]> =
                    blocks.into_iter().map(Vec::into_boxed_slice).collect();
                Ok(Expr(blocks))
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Expr<Var, Val>, A::Error> {
                #[derive(serde::Deserialize)]
                struct ExprFields<Var, Val> {
                    ops: Vec<Vec<Op<Var, Val>>>,
                }
                let fields = ExprFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                #[allow(clippy::type_complexity)]
                let blocks: Arc<[Box<[Op<Var, Val>]>]> =
                    fields.ops.into_iter().map(Vec::into_boxed_slice).collect();
                Ok(Expr(blocks))
            }
        }

        deserializer.deserialize_any(ExprVisitor(PhantomData))
    }
}
