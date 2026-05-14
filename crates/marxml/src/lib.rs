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
mod parse;
mod selector;
mod tokenizer;
mod types;

pub use document::Markdown;
pub use error::ParseError;
pub use parse::{parse, parse_fragment};
pub use selector::{Selector, SelectorError};
pub use types::{ElementRef, SourcePosition, SourceSpan, TextSegments};

/// Crate version exposed for downstream diagnostics.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
