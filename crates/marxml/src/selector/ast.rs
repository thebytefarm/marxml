//! Compiled selector AST. Internal — see [`Selector`](super::Selector) for the
//! public type.

/// A compiled selector. Multiple `Compound`s joined by `,` (union).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct CompiledSelector {
    pub(super) compounds: Vec<Compound>,
}

/// A compound selector: one or more simple selectors joined by combinators.
///
/// e.g. `phase > task` is a compound with two simples
/// (`phase`, `task`) joined by [`Combinator::Child`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Compound {
    /// The rightmost (subject) simple selector.
    pub(super) subject: Simple,
    /// Ancestors, paired with the combinator that leads up to the subject.
    /// Stored right-to-left: `prefix[0]` is the simple immediately to the
    /// left of `subject` (linked by its `combinator`).
    pub(super) prefix: Vec<(Combinator, Simple)>,
}

/// Combinator between two simple selectors in a compound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Combinator {
    /// `a b` — descendant at any depth.
    Descendant,
    /// `a > b` — direct child.
    Child,
}

/// A simple selector: a tag (or `*`) plus attribute/pseudo predicates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Simple {
    /// `None` for `*`.
    pub(super) tag: Option<String>,
    pub(super) predicates: Vec<Predicate>,
}

/// A single attribute or pseudo-class predicate on a simple selector.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Predicate {
    HasAttr(String),
    AttrEquals(String, String),
    AttrStartsWith(String, String),
    AttrEndsWith(String, String),
    AttrContains(String, String),
    FirstChild,
    NthChild(u32),
    Not(Box<Simple>),
}
