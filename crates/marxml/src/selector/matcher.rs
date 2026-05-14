//! Match a [`CompiledSelector`] against a document tree.
//!
//! The matcher walks the tree depth-first, tracking the chain of ancestors so
//! it can evaluate descendant/child combinators. Matches are deduplicated by
//! [`ElementData`] pointer identity so a doc that satisfies multiple compounds
//! in a union doesn't appear twice.

use std::collections::HashSet;

use super::ast::{Combinator, CompiledSelector, Compound, Predicate, Simple};
use crate::types::{ElementData, ElementRef};

#[derive(Clone, Copy)]
struct NodeCtx<'a> {
    data: &'a ElementData,
    /// Position of this node among its siblings (0-based).
    index: u32,
}

pub(crate) fn collect_matches<'a>(
    roots: &'a [ElementData],
    raw: &'a str,
    sel: &CompiledSelector,
) -> Vec<ElementRef<'a>> {
    let mut out: Vec<ElementRef<'a>> = Vec::new();
    let mut seen: HashSet<usize> = HashSet::new();
    let mut ancestors: Vec<NodeCtx<'a>> = Vec::new();
    for (i, root) in roots.iter().enumerate() {
        walk(
            root,
            u32::try_from(i).unwrap_or(u32::MAX),
            sel,
            &mut ancestors,
            raw,
            &mut out,
            &mut seen,
        );
    }
    out
}

fn walk<'a>(
    node: &'a ElementData,
    index: u32,
    sel: &CompiledSelector,
    ancestors: &mut Vec<NodeCtx<'a>>,
    raw: &'a str,
    out: &mut Vec<ElementRef<'a>>,
    seen: &mut HashSet<usize>,
) {
    let ctx = NodeCtx { data: node, index };
    for compound in &sel.compounds {
        if compound_matches(&ctx, compound, ancestors) {
            let key = std::ptr::from_ref(node) as usize;
            if seen.insert(key) {
                out.push(ElementRef { data: node, raw });
            }
            break;
        }
    }
    ancestors.push(ctx);
    for (i, child) in node.children.iter().enumerate() {
        walk(
            child,
            u32::try_from(i).unwrap_or(u32::MAX),
            sel,
            ancestors,
            raw,
            out,
            seen,
        );
    }
    ancestors.pop();
}

fn compound_matches(node: &NodeCtx<'_>, compound: &Compound, ancestors: &[NodeCtx<'_>]) -> bool {
    if !simple_matches(node, &compound.subject) {
        return false;
    }
    // Walk the prefix right-to-left. At each step, we need to find an ancestor
    // that satisfies the next simple, respecting the combinator.
    let mut depth = ancestors.len();
    for (combinator, simple) in &compound.prefix {
        match combinator {
            Combinator::Child => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
                let ancestor = ancestors[depth];
                if !simple_matches(&ancestor, simple) {
                    return false;
                }
            }
            Combinator::Descendant => {
                let mut found = false;
                while depth > 0 {
                    depth -= 1;
                    let ancestor = ancestors[depth];
                    if simple_matches(&ancestor, simple) {
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
        }
    }
    true
}

fn simple_matches(node: &NodeCtx<'_>, simple: &Simple) -> bool {
    if let Some(tag) = &simple.tag {
        if node.data.tag != *tag {
            return false;
        }
    }
    simple.predicates.iter().all(|p| predicate_matches(node, p))
}

fn predicate_matches(node: &NodeCtx<'_>, pred: &Predicate) -> bool {
    match pred {
        Predicate::HasAttr(name) => node.data.attrs.iter().any(|(k, _)| k == name),
        Predicate::AttrEquals(name, value) => {
            attr_value(node.data, name).is_some_and(|v| v == value)
        }
        Predicate::AttrStartsWith(name, value) => {
            attr_value(node.data, name).is_some_and(|v| v.starts_with(value.as_str()))
        }
        Predicate::AttrEndsWith(name, value) => {
            attr_value(node.data, name).is_some_and(|v| v.ends_with(value.as_str()))
        }
        Predicate::AttrContains(name, value) => {
            attr_value(node.data, name).is_some_and(|v| v.contains(value.as_str()))
        }
        Predicate::FirstChild => node.index == 0,
        Predicate::NthChild(n) => node.index.saturating_add(1) == *n,
        Predicate::Not(inner) => !simple_matches(node, inner),
    }
}

fn attr_value<'a>(data: &'a ElementData, name: &str) -> Option<&'a str> {
    data.attrs
        .iter()
        .find(|(k, _)| k == name)
        .map(|(_, v)| v.as_str())
}
