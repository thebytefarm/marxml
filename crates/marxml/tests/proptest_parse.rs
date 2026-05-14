//! Property tests for `marxml::parse`.

use marxml::parse;
use proptest::prelude::*;

proptest! {
    /// Parse must never panic on arbitrary input. It must either return
    /// `Ok` (when the input happens to look like valid markdown+XML) or
    /// `Err` (when it doesn't), but never crash.
    #[test]
    fn parse_never_panics(input in ".{0,256}") {
        let _ = parse(&input);
    }

    /// Parsing a clean, well-formed self-closing tag with an alphanumeric
    /// tag name and quoted attribute always succeeds.
    #[test]
    fn well_formed_self_close_always_parses(
        tag in "[a-z][a-z0-9]{0,15}",
        key in "[a-z][a-z0-9]{0,15}",
        value in "[^\"\\\\\n]{0,32}",
    ) {
        let src = format!(r#"<{tag} {key}="{value}"/>"#);
        let doc = parse(&src).expect("clean self-close must parse");
        prop_assert_eq!(doc.root_count(), 1);
        let el = doc.root_elements().next().unwrap();
        prop_assert_eq!(el.tag(), &tag);
        prop_assert_eq!(el.attr(&key), Some(value.as_str()));
    }

    /// For any valid clean input, `Markdown::raw` echoes the source byte-for-byte.
    #[test]
    fn raw_round_trips(tag in "[a-z][a-z0-9]{0,15}", body in "[^<>&\"\n]{0,64}") {
        let src = format!("<{tag}>{body}</{tag}>");
        let doc = parse(&src).expect("clean tag must parse");
        prop_assert_eq!(doc.raw(), src.as_str());
    }
}
