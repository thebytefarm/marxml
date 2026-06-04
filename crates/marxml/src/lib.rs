//! # marxml
//!
//! Fast markdown + XML query, mutation, validation, and serialization. Rust
//! core for the marxml ecosystem. See <https://github.com/thebytefarm/marxml>.
//!
//! The crate exposes:
//! - [`parse`] / [`parse_fragment`] / [`parse_owned`] — build a [`Markdown`]
//!   document from a source string.
//! - [`Selector`] — CSS-subset selectors compiled once, reused against many
//!   documents.
//! - [`Markdown::update`] / [`Markdown::replace_content`] /
//!   [`Markdown::replace_in`] — string-returning mutation helpers.
//! - [`Schema`] + [`validate`] — declarative validation.
//! - [`Markdown::to_xml`] / [`Markdown::to_json`] +
//!   [`SerializeOpts`] — render the parsed tree back out.
//!
//! ```
//! use marxml::{parse, Selector};
//!
//! let doc = parse(r#"<task id="1" status="todo">write tests</task>"#)?;
//! assert_eq!(doc.root_count(), 1);
//!
//! let sel = Selector::parse(r#"task[status="todo"]"#)?;
//! let updated = doc.update(&sel, &[("status", "done")]);
//! assert!(updated.contains(r#"status="done""#));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

mod document;
mod error;
mod escape;
mod mutate;
mod parse;
pub mod schema;
mod selector;
mod serialize;
mod tokenizer;
mod types;
mod validate;

pub use document::Markdown;
pub use error::{MalformedAttrKind, MalformedTagKind, ParseError};
pub use escape::{escape_attr, escape_text, is_valid_name};
pub use mutate::{MutateError, MutationReport};
pub use parse::{parse, parse_fragment, parse_owned, MAX_DEPTH, MAX_INPUT_BYTES};
pub use schema::{AttrConstraint, AttrKind, Schema, SchemaBuilder, SchemaError, TagBuilder};
pub use selector::{Selector, SelectorError, SyntaxKind};
pub use serialize::SerializeOpts;
pub use types::{ElementRef, SourcePosition, SourceSpan};
pub use validate::{validate, ValidationError, ValidationReport};

/// Crate version exposed for downstream diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
