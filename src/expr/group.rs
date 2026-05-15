use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::expr::traits::ResolveGroup;

/// Function-pointer table mapping a group's `Index` to its concrete `Var`s.
pub type ColumnTable<I, V> = &'static [fn(I) -> V];

/// Iterable relation of correlated-attribute tuples. `col=0` is the
/// implicit `Arg(iter_no)` cell; `COLUMNS` covers `col = 1..N`.
pub trait VarGroup {
    type Var: 'static;
    type Index: Copy + 'static;

    /// Column constructors for cols `1..N`; `COLUMNS[0]` is the primary cell.
    const COLUMNS: ColumnTable<Self::Index, Self::Var>;

    /// Suffix names for cols `2..N`, parallel to `COLUMNS[1..]`.
    const SCHEMA: &'static [&'static str];

    /// Constructor for the `col=0` `ARG` cell (e.g. `Attribute::Arg`).
    fn arg(iter_no: u8) -> Self::Var;

    /// Map a member's short name to row position for `@G(name, …)` masks.
    fn member_by_name(&self, name: &str) -> Option<usize>;

    /// Row count when membership is statically known; `None` for dynamic
    /// groups.
    fn static_size(&self) -> Option<usize> {
        None
    }

    /// Suffix → `col_no`: `"ARG"` → 0, `""` (primary) → 1, else `SCHEMA` + 2.
    fn column_by_suffix(&self, suffix: &str) -> Option<u8> {
        match suffix {
            "ARG" => Some(0),
            "" => Some(1),
            _ => Self::SCHEMA
                .iter()
                .position(|s| *s == suffix)
                .map(|i| (i + 2) as u8),
        }
    }

    /// `Self::COLUMNS` as an instance-call method.
    fn columns(&self) -> ColumnTable<Self::Index, Self::Var> {
        Self::COLUMNS
    }

    /// Build a row from `(iter_no, idx)`: `row[0] = arg(iter_no)`,
    /// `row[k+1] = COLUMNS[k](idx)`. Passable as a fn-pointer to `map`.
    fn make_row((iter_no, idx): (usize, Self::Index)) -> Vec<Self::Var> {
        let mut row = Vec::with_capacity(1 + Self::COLUMNS.len());
        row.push(Self::arg(iter_no as u8));
        row.extend(Self::COLUMNS.iter().map(|f| f(idx)));
        row
    }
}

/// A group with a row-position bitmask; `u32::MAX` allows all rows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarSubgroup<Grp> {
    pub inner: Grp,
    pub mask: u32,
}

impl<Grp> From<Grp> for VarSubgroup<Grp> {
    fn from(inner: Grp) -> Self {
        Self {
            inner,
            mask: u32::MAX,
        }
    }
}

impl<Grp> VarSubgroup<Grp> {
    pub fn masked(inner: Grp, mask: u32) -> Self {
        Self { inner, mask }
    }

    /// True if row position `pos` passes the mask.
    pub fn allows(&self, pos: usize) -> bool {
        if self.mask == u32::MAX {
            return true;
        }
        pos < 32 && (self.mask & (1u32 << pos)) != 0
    }
}

impl<Grp: fmt::Display> fmt::Display for VarSubgroup<Grp> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)?;
        if self.mask != u32::MAX {
            write!(f, "(")?;
            let mut first = true;
            for bit in 0..32u32 {
                if self.mask & (1u32 << bit) != 0 {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "@{}", bit)?;
                    first = false;
                }
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

/// Owned snapshot of a group iteration's rows + current row index.
pub struct GroupCursor<Var> {
    rows: Vec<Vec<Var>>,
    idx: usize,
}

impl<Var> GroupCursor<Var> {
    pub fn new(rows: Vec<Vec<Var>>) -> Self {
        Self { rows, idx: 0 }
    }

    /// Materialise rows via `ctx`, apply mask, rewrite `row[0]` to the
    /// filtered `iter_no`.
    pub fn build<G, Ctx>(ctx: &Ctx, subgrp: &VarSubgroup<G>) -> Self
    where
        G: VarGroup<Var = Var>,
        Ctx: ResolveGroup<G>,
    {
        let rows: Vec<Vec<Var>> = ctx
            .resolve_group(&subgrp.inner)
            .enumerate()
            .filter(|(orig_pos, _)| subgrp.allows(*orig_pos))
            .enumerate()
            .map(|(iter_no, (_, mut row))| {
                if !row.is_empty() {
                    row[0] = G::arg(iter_no as u8);
                }
                row
            })
            .collect();
        Self::new(rows)
    }

    /// True if the cursor currently points at a valid row.
    pub fn is_live(&self) -> bool {
        self.idx < self.rows.len()
    }

    /// Advance to the next row. Returns `true` if the new position is
    /// still within bounds.
    pub fn advance(&mut self) -> bool {
        self.idx += 1;
        self.is_live()
    }

    /// Read the var at column `col` of the current row.
    pub fn col(&self, col: u8) -> Option<&Var> {
        self.rows.get(self.idx)?.get(col as usize)
    }

    /// Current row position (0-based).
    pub fn row_no(&self) -> usize {
        self.idx
    }
}

/// Default no-op group for expressions that have no groups at all.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoGroup<Var = ()>(PhantomData<Var>);

impl<Var> Default for NoGroup<Var> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<Var> fmt::Display for NoGroup<Var> {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

impl<Var> FromStr for NoGroup<Var> {
    type Err = &'static str;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        Err("no groups supported")
    }
}

// `NoGroup` carries no rows; every Context trivially satisfies
// `ResolveGroup<NoGroup<_>>` with an empty iterator.
impl<T: ?Sized, Var: Copy + 'static> ResolveGroup<NoGroup<Var>> for T {
    fn resolve_group<'a>(&'a self, _grp: &NoGroup<Var>) -> Box<dyn Iterator<Item = Vec<Var>> + 'a> {
        Box::new(std::iter::empty())
    }
}

impl<Var: Copy + 'static> VarGroup for NoGroup<Var> {
    type Index = ();
    type Var = Var;

    const COLUMNS: &'static [fn(()) -> Var] = &[];
    const SCHEMA: &'static [&'static str] = &[];

    fn arg(_: u8) -> Var {
        // NoGroup has no rows; `make_row` returns empty without ever
        // touching this. Kept as a panic for safety if reached.
        unreachable!("NoGroup::arg should not be called")
    }

    fn member_by_name(&self, _name: &str) -> Option<usize> {
        None
    }

    fn make_row(_: (usize, ())) -> Vec<Var> {
        Vec::new()
    }
}
