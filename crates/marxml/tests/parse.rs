//! Integration tests for `marxml::parse`.

#![allow(clippy::cast_possible_truncation)]

use marxml::{parse, parse_fragment, ParseError};
use rstest::rstest;

// ─── Happy-path: structure ────────────────────────────────────────────────

#[test]
fn empty_input_parses_to_empty_doc() {
    let doc = parse("").expect("empty input is valid");
    assert_eq!(doc.root_count(), 0);
    assert_eq!(doc.raw(), "");
}

#[test]
fn plain_markdown_has_no_elements() {
    let src = "# Hello\n\nSome paragraph with x < 3 and y > 1.\n";
    let doc = parse(src).expect("plain markdown parses cleanly");
    assert_eq!(doc.root_count(), 0);
    assert_eq!(doc.raw(), src);
}

#[test]
fn single_tag_no_attrs() {
    let doc = parse("<task>do thing</task>").unwrap();
    assert_eq!(doc.root_count(), 1);
    let task = doc.root_elements().next().unwrap();
    assert_eq!(task.tag(), "task");
    assert_eq!(task.attr("id"), None);
    assert_eq!(task.content(), "do thing");
    assert!(!task.is_self_closing());
}

#[test]
fn single_tag_with_attrs_preserved_in_order() {
    let doc = parse(r#"<task id="1" status="todo">body</task>"#).unwrap();
    let task = doc.root_elements().next().unwrap();
    assert_eq!(task.attr("id"), Some("1"));
    assert_eq!(task.attr("status"), Some("todo"));
    let attrs: Vec<_> = task.attrs().collect();
    assert_eq!(attrs, vec![("id", "1"), ("status", "todo")]);
}

#[test]
fn self_closing_tag() {
    let doc = parse(r#"<spacer height="20"/>"#).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.tag(), "spacer");
    assert!(el.is_self_closing());
    assert_eq!(el.content(), "");
    assert_eq!(el.attr("height"), Some("20"));
}

#[test]
fn self_closing_tag_with_space_before_slash() {
    let doc = parse("<divider />").unwrap();
    let el = doc.root_elements().next().unwrap();
    assert!(el.is_self_closing());
    assert_eq!(el.tag(), "divider");
}

#[test]
fn nested_different_tags() {
    let src = r#"<phase id="1"><task id="1.1">a</task></phase>"#;
    let doc = parse(src).unwrap();
    let phase = doc.root_elements().next().unwrap();
    let kids: Vec<_> = phase.children().collect();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].tag(), "task");
    assert_eq!(kids[0].attr("id"), Some("1.1"));
    assert_eq!(kids[0].content(), "a");
}

#[test]
fn same_tag_nesting_works() {
    // The TS implementation could not handle this; marxml does.
    let src = "<a><a>inner</a></a>";
    let doc = parse(src).unwrap();
    let outer = doc.root_elements().next().unwrap();
    let kids: Vec<_> = outer.children().collect();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0].tag(), "a");
    assert_eq!(kids[0].content(), "inner");
}

#[test]
fn deeply_nested_chain() {
    let src = "<a><b><c><d>deep</d></c></b></a>";
    let doc = parse(src).unwrap();
    let a = doc.root_elements().next().unwrap();
    let b = a.children().next().unwrap();
    let c = b.children().next().unwrap();
    let d = c.children().next().unwrap();
    assert_eq!(d.tag(), "d");
    assert_eq!(d.content(), "deep");
}

#[test]
fn mixed_markdown_and_xml_preserves_raw() {
    let src = "# Hello\n\n<task id=\"1\">body</task>\n\nMore text.\n<task id=\"2\"/>\nDone.";
    let doc = parse(src).unwrap();
    assert_eq!(doc.raw(), src);
    assert_eq!(doc.root_count(), 2);
}

#[test]
fn hyphenated_tag_and_attr_names() {
    let doc = parse(r#"<my-task data-id="42"/>"#).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.tag(), "my-task");
    assert_eq!(el.attr("data-id"), Some("42"));
}

#[test]
fn underscore_in_names() {
    let doc = parse(r#"<my_task my_attr="x"/>"#).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.tag(), "my_task");
    assert_eq!(el.attr("my_attr"), Some("x"));
}

#[test]
fn empty_attribute_value() {
    let doc = parse(r#"<task note=""/>"#).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.attr("note"), Some(""));
}

#[test]
fn content_with_unicode_passes_through() {
    let src = "<note>héllo 🎉 café</note>";
    let doc = parse(src).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.content(), "héllo 🎉 café");
}

#[test]
fn attr_value_with_unicode() {
    let src = r#"<note title="日本語"/>"#;
    let doc = parse(src).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.attr("title"), Some("日本語"));
}

// ─── Location tracking ────────────────────────────────────────────────────

#[test]
fn location_tracks_line_and_offset() {
    let src = "first line\n<task id=\"1\">body</task>\nthird";
    let doc = parse(src).unwrap();
    let el = doc.root_elements().next().unwrap();
    let span = el.location();
    assert_eq!(span.start.line, 2);
    assert_eq!(span.start.offset, "first line\n".len() as u32);
    assert_eq!(span.end.line, 2);
}

#[test]
fn nested_element_offsets_are_document_relative() {
    let src = "\n<phase>\n  <task id=\"1\"/>\n</phase>";
    let doc = parse(src).unwrap();
    let phase = doc.root_elements().next().unwrap();
    let task = phase.children().next().unwrap();
    let task_offset = task.location().start.offset as usize;
    assert_eq!(&src[task_offset..task_offset + 13], r#"<task id="1"/"#);
}

#[test]
fn line_counter_advances_across_attr_value_newlines() {
    // Attribute values may span multiple lines.
    let src = "<task summary=\"line one\nline two\">body</task>";
    let doc = parse(src).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.attr("summary"), Some("line one\nline two"));
}

// ─── Error cases ──────────────────────────────────────────────────────────

#[test]
fn unclosed_tag_errors() {
    let err = parse("<task id=\"1\">forgot to close").unwrap_err();
    match err {
        ParseError::UnclosedTag { tag, line } => {
            assert_eq!(tag, "task");
            assert_eq!(line, 1);
        }
        other => panic!("expected UnclosedTag, got {other:?}"),
    }
}

#[test]
fn mismatched_close_errors() {
    let err = parse("<a><b></a></b>").unwrap_err();
    match err {
        ParseError::MismatchedClose {
            found, expected, ..
        } => {
            assert_eq!(found, "a");
            assert_eq!(expected, "b");
        }
        other => panic!("expected MismatchedClose, got {other:?}"),
    }
}

#[test]
fn stray_close_errors() {
    let err = parse("</nothing>").unwrap_err();
    match err {
        ParseError::StrayClose { tag, .. } => {
            assert_eq!(tag, "nothing");
        }
        other => panic!("expected StrayClose, got {other:?}"),
    }
}

#[test]
fn duplicate_id_within_tag_errors() {
    let err = parse(r#"<task id="x"/><task id="x"/>"#).unwrap_err();
    match err {
        ParseError::DuplicateId { tag, id, .. } => {
            assert_eq!(tag, "task");
            assert_eq!(id, "x");
        }
        other => panic!("expected DuplicateId, got {other:?}"),
    }
}

#[test]
fn duplicate_id_across_different_tags_is_allowed() {
    let doc = parse(r#"<task id="1"/><phase id="1"/>"#).unwrap();
    assert_eq!(doc.root_count(), 2);
}

#[test]
fn duplicate_id_through_nesting_still_errors() {
    let err = parse(r#"<task id="1"><task id="1"/></task>"#).unwrap_err();
    assert!(matches!(err, ParseError::DuplicateId { .. }));
}

#[rstest]
#[case::missing_quote_open("<task id=1>", "expected '\"'")]
#[case::missing_equals("<task id>", "expected '='")]
#[case::unterminated_value(r#"<task id="x"#, "unterminated value")]
#[case::no_terminator("<task", "not terminated")]
#[case::bad_self_close("<task /", "expected '>'")]
fn malformed_tag_reports_useful_reason(#[case] src: &str, #[case] expected_fragment: &str) {
    let err = parse(src).unwrap_err();
    let message = err.to_string();
    assert!(
        message.contains(expected_fragment),
        "expected error containing {expected_fragment:?}, got {message:?}"
    );
}

#[test]
fn unterminated_close_tag_errors() {
    let err = parse("<a></a ").unwrap_err();
    assert!(matches!(err, ParseError::MalformedTag { .. }));
}

#[test]
fn parse_error_line_accessor_unclosed() {
    let err = parse("line one\nline two\n<task>").unwrap_err();
    assert_eq!(err.line(), 3);
}

#[test]
fn parse_error_line_accessor_mismatched_close() {
    let err = parse("\n<a><b></a></b>").unwrap_err();
    assert!(matches!(err, ParseError::MismatchedClose { .. }));
    assert_eq!(err.line(), 2);
}

#[test]
fn parse_error_line_accessor_stray_close() {
    let err = parse("\n</nope>").unwrap_err();
    assert!(matches!(err, ParseError::StrayClose { .. }));
    assert_eq!(err.line(), 2);
}

#[test]
fn parse_error_line_accessor_malformed_tag() {
    let err = parse("\n<task").unwrap_err();
    assert!(matches!(err, ParseError::MalformedTag { .. }));
    assert_eq!(err.line(), 2);
}

#[test]
fn parse_error_line_accessor_malformed_attribute() {
    let err = parse("\n<task id=>").unwrap_err();
    assert!(matches!(err, ParseError::MalformedAttribute { .. }));
    assert_eq!(err.line(), 2);
}

#[test]
fn parse_error_line_accessor_duplicate_id() {
    let err = parse("\n<task id=\"x\"/>\n<task id=\"x\"/>").unwrap_err();
    assert!(matches!(err, ParseError::DuplicateId { .. }));
    assert_eq!(err.line(), 3);
}

#[test]
fn newline_inside_tag_whitespace_tracks_line() {
    // `\n` between tag name and an attribute name is whitespace `skip_ws`
    // walks over; line counter must advance.
    let src = "<task\n  id=\"1\">body</task>";
    let doc = parse(src).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(el.attr("id"), Some("1"));
    // Element span starts on line 1 (where `<` is) and ends on line 2.
    let span = el.location();
    assert_eq!(span.start.line, 1);
    assert_eq!(span.end.line, 2);
}

// ─── Non-tag characters survive ───────────────────────────────────────────

#[rstest]
#[case::less_than_in_prose("if x < 3 then y > 0")]
#[case::digit_after_lt("a < 2b")]
#[case::empty_brackets("< > are not a tag")]
#[case::space_after_lt("< not-a-tag>")]
fn lone_lt_is_treated_as_text(#[case] src: &str) {
    let doc = parse(src).expect("text-shaped input must parse cleanly");
    assert_eq!(doc.root_count(), 0);
    assert_eq!(doc.raw(), src);
}

#[test]
fn parse_fragment_behaves_like_parse() {
    let src = "<note>hi</note>";
    let a = parse(src).unwrap();
    let b = parse_fragment(src).unwrap();
    assert_eq!(a.root_count(), b.root_count());
    assert_eq!(a.raw(), b.raw());
}
