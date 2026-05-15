//! # marxml
//!
//! Fast markdown + XML query and mutation. Rust core for the marxml ecosystem.
//!
//! This release exposes the parsing primitives. Selectors and mutation land
//! in subsequent phases. See <https://github.com/thebytefarm/marxml>.
//!
//! ```
//! let doc = marxml::parse("<task id=\"1\">hello</task>")?;
//! assert_eq!(doc.root_count(), 1);
//! # Ok::<(), marxml::ParseError>(())
//! ```

#![doc(html_root_url = "https://docs.rs/marxml/0.0.0")]

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
pub use error::ParseError;
pub use escape::{escape_attr, escape_text, is_valid_name};
pub use mutate::{MutateError, MutationReport};
pub use parse::{parse, parse_fragment, MAX_DEPTH, MAX_INPUT_BYTES};
pub use schema::{Schema, SchemaError};
pub use selector::{Selector, SelectorError};
pub use serialize::SerializeOpts;
pub use types::{ElementRef, SourcePosition, SourceSpan};
pub use validate::{validate, ValidationError, ValidationReport};

/// Crate version exposed for downstream diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
