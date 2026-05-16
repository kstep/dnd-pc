use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::{expr::traits::ResolveGroup, model::write_maybe_quoted};

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

    /// Inverse of `member_by_name`: short name of the member at `pos`.
    /// Default `None` for groups without name-keyed masks.
    fn member_name(&self, _pos: usize) -> Option<&'static str> {
        None
    }

    /// Extract a row's identifying name (for dynamic groups whose
    /// membership is keyed by name rather than position). Default
    /// `None` — static groups don't need this.
    fn name_of(&self, _row: &[Self::Var]) -> Option<&'static str> {
        None
    }

    /// Inverse of `name_of`: lift an intern'd name into the group's
    /// `Index` type. Lets `SubgroupMask::Names` materialise rows from
    /// the allowlist without consulting Context (e.g. to grant tools the
    /// character doesn't yet have). Default `None`.
    fn index_from_name(&self, _name: &'static str) -> Option<Self::Index> {
        None
    }

    /// Build a row from a mask name at the given iter position. Default
    /// composes `index_from_name` + `make_row`; sum-enum dispatchers
    /// override to route to the concrete variant whose `make_row`
    /// produces the right shape.
    fn materialize_from_name(&self, name: &'static str, iter_no: usize) -> Option<Vec<Self::Var>> {
        let idx = self.index_from_name(name)?;
        Some(Self::make_row((iter_no, idx)))
    }

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

/// Subset selector for a subgroup. Static groups use bit positions in
/// `VARIANTS`; dynamic groups (e.g. `@TOOL`) use a `\0`-joined name
/// allowlist interned via `model::intern` so the slice stays `Copy`.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize)]
pub enum SubgroupMask {
    /// Bit positions in `VARIANTS`. `u32::MAX` = allow all rows.
    Bits(u32),
    /// `\0`-joined name allowlist for runtime-keyed groups.
    Names(&'static str),
}

impl SubgroupMask {
    /// Number of rows this mask admits. `Bits(u32::MAX)` falls back to
    /// the group's `static_size`; explicit bits popcount, names list
    /// length otherwise.
    pub fn admitted_count(&self, static_size: impl FnOnce() -> Option<usize>) -> Option<usize> {
        match self {
            Self::Bits(u32::MAX) => static_size(),
            Self::Bits(bits) => Some(bits.count_ones() as usize),
            Self::Names(joined) => Some(joined.split('\0').count()),
        }
    }
}

impl<'de> Deserialize<'de> for SubgroupMask {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        enum Wire {
            Bits(u32),
            Names(String),
        }
        Ok(match Wire::deserialize(d)? {
            Wire::Bits(bits) => Self::Bits(bits),
            Wire::Names(joined) => Self::Names(crate::model::intern(&joined)),
        })
    }
}

/// A group with a mask narrowing the iterated rows.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarSubgroup<Grp> {
    pub inner: Grp,
    pub mask: SubgroupMask,
}

impl<Grp> From<Grp> for VarSubgroup<Grp> {
    fn from(inner: Grp) -> Self {
        Self {
            inner,
            mask: SubgroupMask::Bits(u32::MAX),
        }
    }
}

impl<Grp> VarSubgroup<Grp> {
    pub fn masked(inner: Grp, bits: u32) -> Self {
        Self {
            inner,
            mask: SubgroupMask::Bits(bits),
        }
    }

    pub fn masked_names(inner: Grp, names: &'static str) -> Self {
        Self {
            inner,
            mask: SubgroupMask::Names(names),
        }
    }

    /// True if `row` (at original position `pos`) passes the mask.
    pub fn allows_row(&self, row: &[Grp::Var], pos: usize) -> bool
    where
        Grp: VarGroup,
    {
        match self.mask {
            SubgroupMask::Bits(u32::MAX) => true,
            SubgroupMask::Bits(bits) => pos < 32 && bits & (1u32 << pos) != 0,
            SubgroupMask::Names(joined) => match self.inner.name_of(row) {
                Some(name) => joined.split('\0').any(|allowed| allowed == name),
                None => false,
            },
        }
    }
}

impl<Grp: fmt::Display + VarGroup> fmt::Display for VarSubgroup<Grp> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.inner)?;
        match self.mask {
            SubgroupMask::Bits(u32::MAX) => Ok(()),
            SubgroupMask::Bits(bits) => {
                write!(f, "(")?;
                let mut first = true;
                for bit in 0..32u32 {
                    if bits & (1u32 << bit) != 0 {
                        if !first {
                            write!(f, ", ")?;
                        }
                        match self.inner.member_name(bit as usize) {
                            Some(name) => write_maybe_quoted(name, f)?,
                            None => write!(f, "@{bit}")?,
                        }
                        first = false;
                    }
                }
                write!(f, ")")
            }
            SubgroupMask::Names(joined) => {
                write!(f, "(")?;
                let mut first = true;
                for name in joined.split('\0') {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write_maybe_quoted(name, f)?;
                    first = false;
                }
                write!(f, ")")
            }
        }
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

    /// Materialise rows for `subgrp`.
    ///
    /// * `SubgroupMask::Names`: iterate the allowlist directly via
    ///   `index_from_name` — bypasses `ctx` so an empty character state doesn't
    ///   shadow the requested tools.
    /// * Otherwise: pull from `ctx.resolve_group`, filter by mask, re-index
    ///   `row[0]` to the filtered `iter_no`.
    pub fn build<G, Ctx>(ctx: &Ctx, subgrp: &VarSubgroup<G>) -> Self
    where
        G: VarGroup<Var = Var>,
        Ctx: ResolveGroup<G>,
    {
        if let SubgroupMask::Names(joined) = subgrp.mask {
            let rows: Vec<Vec<Var>> = joined
                .split('\0')
                .enumerate()
                .filter_map(|(iter_no, name)| subgrp.inner.materialize_from_name(name, iter_no))
                .collect();
            return Self::new(rows);
        }
        let rows: Vec<Vec<Var>> = ctx
            .resolve_group(&subgrp.inner)
            .enumerate()
            .filter(|(orig_pos, row)| subgrp.allows_row(row, *orig_pos))
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
