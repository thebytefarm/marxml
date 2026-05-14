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

use std::collections::BTreeMap;

use regex::Regex;

/// A complete schema — a mapping from tag name to per-tag validation rules.
///
/// Tags that don't appear in the schema are simply not validated. Use the
/// fluent [`Schema::builder`] API to construct one.
#[derive(Debug, Clone)]
pub struct Schema {
    pub(crate) tags: BTreeMap<String, TagSchema>,
}

impl Schema {
    /// Begin building a new schema. See module-level docs for an example.
    #[must_use]
    pub fn builder() -> SchemaBuilder {
        SchemaBuilder {
            tags: BTreeMap::new(),
        }
    }
}

/// Per-tag validation rules.
#[derive(Debug, Clone, Default)]
pub struct TagSchema {
    pub(crate) attrs: BTreeMap<String, AttrConstraint>,
    pub(crate) children_required: Vec<String>,
    pub(crate) children_optional: Vec<String>,
    /// `true` when `children_required` and `children_optional` together
    /// define the *allowlist* of children. `false` (the default) allows any
    /// child unless explicitly required missing.
    pub(crate) children_exclusive: bool,
    pub(crate) content_required: bool,
}

/// What kind of value an attribute should hold.
#[derive(Debug, Clone)]
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

/// A single attribute slot in a [`TagSchema`].
#[derive(Debug, Clone)]
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

/// Builder for [`Schema`].
pub struct SchemaBuilder {
    tags: BTreeMap<String, TagSchema>,
}

impl SchemaBuilder {
    /// Add a tag to the schema, configuring it via the supplied closure.
    #[must_use]
    pub fn tag<F: FnOnce(TagBuilder) -> TagBuilder>(mut self, name: &str, f: F) -> Self {
        let builder = TagBuilder {
            schema: TagSchema::default(),
        };
        let tag_schema = f(builder).schema;
        self.tags.insert(name.to_string(), tag_schema);
        self
    }

    /// Finalize the schema.
    ///
    /// # Panics
    ///
    /// Panics if any `AttrKind::Regex(...)` in the schema contains an invalid
    /// pattern. Schemas are authored once at startup; failing early is
    /// preferred over deferring the error to validation time.
    #[must_use]
    pub fn build(self) -> Schema {
        // Pre-compile any regex attribute kinds to validate their syntax.
        for (tag, ts) in &self.tags {
            for (attr, c) in &ts.attrs {
                if let AttrKind::Regex(pat) = &c.kind {
                    Regex::new(pat).unwrap_or_else(|e| {
                        panic!("invalid regex for {tag}.{attr}: {e}");
                    });
                }
            }
        }
        Schema { tags: self.tags }
    }
}

/// Builder for a single tag's rules within a [`Schema`].
pub struct TagBuilder {
    schema: TagSchema,
}

impl TagBuilder {
    /// Add or replace an attribute constraint.
    #[must_use]
    pub fn attr(mut self, name: &str, constraint: impl Into<AttrConstraint>) -> Self {
        self.schema
            .attrs
            .insert(name.to_string(), constraint.into());
        self
    }

    /// Mark a child tag as required (must appear at least once).
    #[must_use]
    pub fn child_required(mut self, name: &str) -> Self {
        self.schema.children_required.push(name.to_string());
        self
    }

    /// Add a child tag to the allowlist without requiring it.
    #[must_use]
    pub fn child_optional(mut self, name: &str) -> Self {
        self.schema.children_optional.push(name.to_string());
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

    /// Require non-whitespace content inside the element.
    #[must_use]
    pub fn content_required(mut self) -> Self {
        self.schema.content_required = true;
        self
    }
}
