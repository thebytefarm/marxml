//! Declarative schema for validating parsed documents.
//!
//! Build a [`Schema`] with [`Schema::builder`], pass it to
//! [`crate::validate`] along with a parsed [`crate::Markdown`], and inspect
//! the [`crate::ValidationReport`] for any problems.
//!
//! ```
//! use marxml::{schema::AttrKind, Schema};
//!
//! let schema = Schema::builder()
//!     .tag("task", |t| {
//!         t.attr("id", AttrKind::String.required())
//!             .attr("status", AttrKind::Enum(vec!["todo".into(), "done".into()]))
//!             .child_required("status")
//!             .content_required()
//!     })
//!     .build();
//! # let _ = schema;
//! ```

use std::collections::{BTreeMap, BTreeSet};

use regex::Regex;
use thiserror::Error;

use crate::escape::is_valid_name;

/// A complete schema — a mapping from tag name to per-tag validation rules.
///
/// Tags that don't appear in the schema are simply not validated. Use the
/// fluent [`Schema::builder`] API to construct one.
///
/// Internally the schema stores a compiled form of every rule (regex
/// constraints are compiled once, child-allow lists are converted to sets)
/// so [`crate::validate`] can run in O(elements) without per-element
/// recompilation.
#[derive(Debug, Clone, Default)]
pub struct Schema {
    pub(crate) tags: BTreeMap<String, CompiledTagSchema>,
}

impl Schema {
    /// Begin building a new schema. See module-level docs for an example.
    #[must_use]
    pub fn builder() -> SchemaBuilder {
        SchemaBuilder {
            tags: BTreeMap::new(),
            duplicates: Vec::new(),
        }
    }
}

/// Per-tag validation rules — author-facing builder form. The schema is
/// compiled into a private `CompiledTagSchema` on [`SchemaBuilder::build`].
///
/// Constructed only via [`SchemaBuilder::tag`] and the closure-based
/// [`TagBuilder`] API; this struct has no public constructor of its own.
#[derive(Debug, Clone, Default)]
pub(crate) struct TagSchema {
    pub(crate) attrs: BTreeMap<String, AttrConstraint>,
    pub(crate) children_required: Vec<String>,
    pub(crate) children_optional: Vec<String>,
    /// `true` when `children_required` and `children_optional` together
    /// define the *allowlist* of children. `false` (the default) allows any
    /// child unless explicitly required missing.
    pub(crate) children_exclusive: bool,
    pub(crate) content_required: bool,
    /// Attribute names registered more than once on the builder. Surfaces
    /// at `try_build` so silent constraint overwrites become a build error.
    pub(crate) duplicate_attrs: Vec<String>,
}

/// What kind of value an attribute should hold.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrKind {
    /// Any string. No further constraint.
    String,
    /// One of the listed values.
    Enum(Vec<String>),
    /// Match the given regular expression. The pattern is compiled when the
    /// schema is built — invalid regexes panic at build time, which is what
    /// you want for schemas (they're authored once at startup).
    Regex(String),
}

impl AttrKind {
    /// Build an [`AttrKind::Enum`] from any iterable of string-like values.
    /// Lets callers write `AttrKind::one_of(["todo", "done"])` instead of
    /// `AttrKind::Enum(vec!["todo".into(), "done".into()])`.
    #[must_use]
    pub fn one_of<I, S>(values: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self::Enum(values.into_iter().map(Into::into).collect())
    }

    /// Mark this attribute as required.
    #[must_use]
    pub fn required(self) -> AttrConstraint {
        AttrConstraint {
            kind: self,
            required: true,
        }
    }

    /// Mark this attribute as optional. Same as `.into()`.
    #[must_use]
    pub fn optional(self) -> AttrConstraint {
        AttrConstraint {
            kind: self,
            required: false,
        }
    }
}

/// A single attribute slot built by [`TagBuilder::attr`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrConstraint {
    pub(crate) kind: AttrKind,
    pub(crate) required: bool,
}

impl From<AttrKind> for AttrConstraint {
    fn from(kind: AttrKind) -> Self {
        Self {
            kind,
            required: false,
        }
    }
}

/// Compiled per-tag schema — what [`crate::validate`] actually consumes.
///
/// Regexes are pre-compiled and child-name lists are converted to sets so
/// validation runs in time proportional to the document, not the schema.
#[derive(Debug, Clone)]
pub(crate) struct CompiledTagSchema {
    pub(crate) attrs: BTreeMap<String, CompiledAttrConstraint>,
    pub(crate) children_required: Vec<String>,
    /// `children_required + children_optional`, deduplicated. Used for
    /// O(log n) allowlist membership in [`Self::children_exclusive`] checks.
    pub(crate) children_allowed: BTreeSet<String>,
    pub(crate) children_exclusive: bool,
    pub(crate) content_required: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct CompiledAttrConstraint {
    pub(crate) kind: CompiledAttrKind,
    pub(crate) required: bool,
}

#[derive(Debug, Clone)]
pub(crate) enum CompiledAttrKind {
    String,
    /// Allowed values, deduplicated into a sorted set so membership checks
    /// are O(log n) instead of O(n) per validated attribute.
    Enum(BTreeSet<String>),
    Regex(Regex),
}

/// Reasons [`SchemaBuilder::try_build`] can reject a schema.
///
/// `Eq` is intentionally not derived because [`SchemaError::InvalidRegex`]
/// wraps a [`regex::Error`], which is `PartialEq` but not `Eq`. Use
/// `matches!` or `PartialEq` when comparing variants in tests.
#[derive(Debug, Clone, Error, PartialEq)]
#[non_exhaustive]
pub enum SchemaError {
    /// An `AttrKind::Regex(...)` pattern failed to compile. The underlying
    /// [`regex::Error`] is exposed via [`std::error::Error::source`] so
    /// downstream renderers (`anyhow`, `eyre`, `tracing`) can walk the
    /// cause chain.
    #[error("invalid regex for {tag}.{attr}")]
    InvalidRegex {
        /// Tag carrying the offending attribute.
        tag: String,
        /// Attribute name.
        attr: String,
        /// Underlying compiler error from the `regex` crate.
        #[source]
        source: regex::Error,
    },
    /// A tag, attribute, or child name in the schema is not a valid XML name.
    ///
    /// Schemas that name elements the parser cannot produce would silently
    /// never apply; rejecting them up front turns the dead rule into a build
    /// error.
    #[error("invalid XML name in schema: {scope} {name:?}")]
    InvalidName {
        /// What kind of name was rejected (`"tag"`, `"attr"`, `"child"`).
        scope: &'static str,
        /// The offending name.
        name: String,
    },
    /// The same tag name was registered more than once on the builder.
    ///
    /// Silently overwriting earlier rules weakens validation in a way that's
    /// hard to notice; the builder rejects the second registration instead.
    #[error("duplicate tag {tag:?} in schema")]
    DuplicateTag {
        /// Tag name that appeared twice.
        tag: String,
    },
    /// The same attribute name was registered more than once on a tag.
    ///
    /// Same reasoning as [`SchemaError::DuplicateTag`] — a later constraint
    /// silently disabling an earlier one is a hidden weakening of the schema.
    #[error("duplicate attribute {attr:?} on tag {tag:?}")]
    DuplicateAttr {
        /// Tag carrying the duplicate attribute.
        tag: String,
        /// Attribute name that appeared twice.
        attr: String,
    },
}

/// Builder for [`Schema`].
#[derive(Debug, Clone, Default)]
pub struct SchemaBuilder {
    tags: BTreeMap<String, TagSchema>,
    /// Names registered more than once. Surfaces at `try_build` so silent
    /// overwrites (a builder helper that re-registers the same tag) become
    /// a build error instead of weakening validation behind the caller's back.
    duplicates: Vec<String>,
}

impl SchemaBuilder {
    /// Add a tag to the schema, configuring it via the supplied closure.
    ///
    /// Registering the same tag name twice is a build error; a second
    /// registration replaces the first internally but causes `try_build` to
    /// return [`SchemaError::DuplicateTag`].
    #[must_use]
    pub fn tag<F>(mut self, name: impl Into<String>, f: F) -> Self
    where
        F: FnOnce(TagBuilder) -> TagBuilder,
    {
        let name = name.into();
        let builder = TagBuilder {
            schema: TagSchema::default(),
        };
        let tag_schema = f(builder).schema;
        if self.tags.insert(name.clone(), tag_schema).is_some() {
            self.duplicates.push(name);
        }
        self
    }

    /// Finalize the schema. Compiles every `AttrKind::Regex(...)` pattern,
    /// converts enums to sets, and pre-builds the child-allow sets used by
    /// exclusive-children validation.
    ///
    /// # Panics
    ///
    /// Panics on any [`SchemaError`] produced by [`Self::try_build`]:
    /// invalid regex patterns ([`SchemaError::InvalidRegex`]), duplicate
    /// tag or attribute registrations ([`SchemaError::DuplicateTag`],
    /// [`SchemaError::DuplicateAttr`]), or names that aren't valid XML
    /// names ([`SchemaError::InvalidName`]). Use [`Self::try_build`] for a
    /// fallible variant when the schema is loaded from a config file or
    /// other runtime source.
    #[must_use]
    pub fn build(self) -> Schema {
        self.try_build()
            .unwrap_or_else(|e| panic!("schema build failed: {e}"))
    }

    /// Finalize the schema, returning an error instead of panicking on
    /// invalid input. The recoverable counterpart of [`Self::build`].
    ///
    /// # Errors
    ///
    /// - [`SchemaError::InvalidRegex`] when an `AttrKind::Regex(...)`
    ///   pattern fails to compile.
    /// - [`SchemaError::DuplicateTag`] when the same tag was registered
    ///   more than once on the builder.
    /// - [`SchemaError::DuplicateAttr`] when the same attribute was
    ///   registered more than once on a tag.
    /// - [`SchemaError::InvalidName`] when a tag, attribute, or child
    ///   name in the schema is not a valid XML name.
    pub fn try_build(self) -> Result<Schema, SchemaError> {
        if let Some(dup) = self.duplicates.into_iter().next() {
            return Err(SchemaError::DuplicateTag { tag: dup });
        }
        let mut tags = BTreeMap::new();
        for (tag, ts) in self.tags {
            tags.insert(tag.clone(), compile_tag(&tag, ts)?);
        }
        Ok(Schema { tags })
    }
}

fn compile_tag(tag: &str, ts: TagSchema) -> Result<CompiledTagSchema, SchemaError> {
    if !is_valid_name(tag) {
        return Err(SchemaError::InvalidName {
            scope: "tag",
            name: tag.to_string(),
        });
    }
    if let Some(dup) = ts.duplicate_attrs.into_iter().next() {
        return Err(SchemaError::DuplicateAttr {
            tag: tag.to_string(),
            attr: dup,
        });
    }
    let mut attrs = BTreeMap::new();
    for (name, c) in ts.attrs {
        if !is_valid_name(&name) {
            return Err(SchemaError::InvalidName {
                scope: "attr",
                name,
            });
        }
        let kind = match c.kind {
            AttrKind::String => CompiledAttrKind::String,
            AttrKind::Enum(values) => CompiledAttrKind::Enum(values.into_iter().collect()),
            AttrKind::Regex(pat) => {
                // Anchor the pattern so the constraint is "value matches the
                // whole pattern", not "value contains a substring matching
                // the pattern". Without this, `Regex("todo|done")` would
                // accept `"undone"` because `is_match` searches anywhere in
                // the haystack.
                let anchored = format!("\\A(?:{pat})\\z");
                let re = Regex::new(&anchored).map_err(|source| SchemaError::InvalidRegex {
                    tag: tag.to_string(),
                    attr: name.clone(),
                    source,
                })?;
                CompiledAttrKind::Regex(re)
            }
        };
        attrs.insert(
            name,
            CompiledAttrConstraint {
                kind,
                required: c.required,
            },
        );
    }
    let mut children_allowed: BTreeSet<String> = BTreeSet::new();
    for name in &ts.children_required {
        if !is_valid_name(name) {
            return Err(SchemaError::InvalidName {
                scope: "child",
                name: name.clone(),
            });
        }
        children_allowed.insert(name.clone());
    }
    for name in &ts.children_optional {
        if !is_valid_name(name) {
            return Err(SchemaError::InvalidName {
                scope: "child",
                name: name.clone(),
            });
        }
        children_allowed.insert(name.clone());
    }
    // De-dupe required children so a builder that lists the same required
    // tag twice doesn't produce two `MissingChild` errors per element.
    let mut seen_required: BTreeSet<String> = BTreeSet::new();
    let children_required: Vec<String> = ts
        .children_required
        .into_iter()
        .filter(|n| seen_required.insert(n.clone()))
        .collect();
    Ok(CompiledTagSchema {
        attrs,
        children_required,
        children_allowed,
        children_exclusive: ts.children_exclusive,
        content_required: ts.content_required,
    })
}

/// Builder for a single tag's rules within a [`Schema`].
#[derive(Debug, Clone, Default)]
pub struct TagBuilder {
    schema: TagSchema,
}

impl TagBuilder {
    /// Add an attribute constraint. Registering the same attribute name
    /// twice on a tag is a build error (see [`SchemaError::DuplicateAttr`]).
    #[must_use]
    pub fn attr(mut self, name: impl Into<String>, constraint: impl Into<AttrConstraint>) -> Self {
        let name = name.into();
        if self
            .schema
            .attrs
            .insert(name.clone(), constraint.into())
            .is_some()
        {
            self.schema.duplicate_attrs.push(name);
        }
        self
    }

    /// Mark a child tag as required (must appear at least once).
    #[must_use]
    pub fn child_required(mut self, name: impl Into<String>) -> Self {
        self.schema.children_required.push(name.into());
        self
    }

    /// Add a child tag to the allowlist without requiring it.
    #[must_use]
    pub fn child_optional(mut self, name: impl Into<String>) -> Self {
        self.schema.children_optional.push(name.into());
        self
    }

    /// Switch the children spec to "exclusive": only tags in
    /// `child_required` or `child_optional` are permitted. Children not in
    /// either list trigger an `UnexpectedChild` error.
    #[must_use]
    pub fn exclusive_children(mut self) -> Self {
        self.schema.children_exclusive = true;
        self
    }

    /// Require non-whitespace text content directly inside the element.
    ///
    /// Child-element markup does not count — `<task><status/></task>` does
    /// not satisfy `content_required` even though its raw inner content is
    /// non-empty. Use this when the rule is "the element must carry text the
    /// user wrote", not "the element must have *something* inside it".
    #[must_use]
    pub fn content_required(mut self) -> Self {
        self.schema.content_required = true;
        self
    }
}
