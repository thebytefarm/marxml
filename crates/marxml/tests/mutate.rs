//! Integration tests for the mutation API.

use marxml::{parse, Selector};
use regex::Regex;

fn run_update(src: &str, sel: &str, attrs: &[(&str, &str)]) -> String {
    let doc = parse(src).unwrap();
    let s = Selector::parse(sel).unwrap();
    doc.update(&s, attrs)
}

fn run_replace_content(src: &str, sel: &str, new_body: &str) -> String {
    let doc = parse(src).unwrap();
    let s = Selector::parse(sel).unwrap();
    doc.replace_content(&s, new_body)
}

fn run_replace_in(src: &str, sel: &str, pattern: &str, replacement: &str) -> String {
    let doc = parse(src).unwrap();
    let s = Selector::parse(sel).unwrap();
    let re = Regex::new(pattern).unwrap();
    doc.replace_in(&s, &re, replacement)
}

// ─── update ────────────────────────────────────────────────────────────────

#[test]
fn update_replaces_existing_attribute() {
    let src = r#"<task id="1" status="todo"/>"#;
    let out = run_update(src, "task", &[("status", "done")]);
    assert_eq!(out, r#"<task id="1" status="done"/>"#);
}

#[test]
fn update_appends_new_attribute() {
    let src = r#"<task id="1"/>"#;
    let out = run_update(src, "task", &[("status", "done")]);
    assert_eq!(out, r#"<task id="1" status="done"/>"#);
}

#[test]
fn update_combines_replace_and_append() {
    let src = r#"<task id="1" status="todo"/>"#;
    let out = run_update(src, "task", &[("status", "done"), ("priority", "high")]);
    assert_eq!(out, r#"<task id="1" status="done" priority="high"/>"#);
}

#[test]
fn update_no_match_returns_raw_unchanged() {
    let src = r#"<task id="1"/>"#;
    let out = run_update(src, "missing", &[("status", "done")]);
    assert_eq!(out, src);
}

#[test]
fn update_applies_to_every_match() {
    let src = r#"<task id="1"/><task id="2"/>"#;
    let out = run_update(src, "task", &[("status", "done")]);
    assert_eq!(
        out,
        r#"<task id="1" status="done"/><task id="2" status="done"/>"#
    );
}

#[test]
fn update_preserves_untouched_bytes_byte_for_byte() {
    let src = "# Heading\n\n<phase id=\"1\" status=\"todo\">\n\nBody text with x < 3.\n\n<task id=\"1.1\"/>\n\n</phase>\n\nTrailing paragraph.";
    let out = run_update(src, "task", &[("status", "done")]);
    // Everything except the `<task>` line should match byte-for-byte.
    let expected = src.replace("<task id=\"1.1\"/>", "<task id=\"1.1\" status=\"done\"/>");
    assert_eq!(out, expected);
}

#[test]
fn update_on_open_close_pair() {
    let src = r#"<task id="1">body</task>"#;
    let out = run_update(src, "task", &[("status", "done")]);
    assert_eq!(out, r#"<task id="1" status="done">body</task>"#);
}

#[test]
fn update_on_tag_with_no_attrs() {
    let src = "<task>body</task>";
    let out = run_update(src, "task", &[("id", "1")]);
    assert_eq!(out, r#"<task id="1">body</task>"#);
}

#[test]
fn update_nested_element_only_targets_match() {
    let src = r#"<phase id="1"><task id="1.1"/></phase>"#;
    let out = run_update(src, "task", &[("status", "done")]);
    assert_eq!(
        out,
        r#"<phase id="1"><task id="1.1" status="done"/></phase>"#
    );
}

#[test]
fn update_with_complex_selector() {
    let src = r#"<task id="1.1"/><task id="1.2"/><task id="2.1"/>"#;
    let out = run_update(src, r#"task[id^="1."]"#, &[("status", "done")]);
    assert_eq!(
        out,
        r#"<task id="1.1" status="done"/><task id="1.2" status="done"/><task id="2.1"/>"#
    );
}

// ─── replace_content ───────────────────────────────────────────────────────

#[test]
fn replace_content_replaces_body() {
    let src = r#"<task id="1">old body</task>"#;
    let out = run_replace_content(src, "task", "new body");
    assert_eq!(out, r#"<task id="1">new body</task>"#);
}

#[test]
fn replace_content_multiline_body() {
    let src = "<task>old\nold</task>";
    let out = run_replace_content(src, "task", "fresh\nfresh\nfresh");
    assert_eq!(out, "<task>fresh\nfresh\nfresh</task>");
}

#[test]
fn replace_content_no_match() {
    let src = "<task>body</task>";
    let out = run_replace_content(src, "phase", "x");
    assert_eq!(out, src);
}

#[test]
fn replace_content_self_close_is_noop() {
    let src = "<spacer/>";
    let out = run_replace_content(src, "spacer", "irrelevant");
    assert_eq!(out, src);
}

#[test]
fn replace_content_multiple_matches() {
    let src = "<note>a</note><note>b</note>";
    let out = run_replace_content(src, "note", "x");
    assert_eq!(out, "<note>x</note><note>x</note>");
}

#[test]
fn replace_content_preserves_surrounding_text() {
    let src = "intro\n\n<note>old</note>\n\nouter";
    let out = run_replace_content(src, "note", "new");
    assert_eq!(out, "intro\n\n<note>new</note>\n\nouter");
}

// ─── replace_in ────────────────────────────────────────────────────────────

#[test]
fn replace_in_regex_replaces_within_content() {
    let src = "<task><status>todo</status></task>";
    let out = run_replace_in(
        src,
        "task",
        r"<status>todo</status>",
        "<status>done</status>",
    );
    assert_eq!(out, "<task><status>done</status></task>");
}

#[test]
fn replace_in_no_pattern_match_is_noop() {
    let src = "<task>body</task>";
    let out = run_replace_in(src, "task", "missing", "x");
    assert_eq!(out, src);
}

#[test]
fn replace_in_no_element_match_is_noop() {
    let src = "<task>body</task>";
    let out = run_replace_in(src, "phase", "body", "x");
    assert_eq!(out, src);
}

#[test]
fn replace_in_replaces_all_pattern_occurrences_in_one_element() {
    let src = "<note>foo foo foo</note>";
    let out = run_replace_in(src, "note", "foo", "bar");
    assert_eq!(out, "<note>bar bar bar</note>");
}

#[test]
fn replace_in_works_with_complex_regex() {
    let src = r#"<task id="1"><status>todo</status></task>"#;
    let out = run_replace_in(
        src,
        r#"task[id="1"]"#,
        r"<status>(todo|done)</status>",
        "<status>skip</status>",
    );
    assert_eq!(out, r#"<task id="1"><status>skip</status></task>"#);
}

#[test]
fn replace_in_targets_multiple_elements() {
    let src = "<note>foo</note><note>foo</note>";
    let out = run_replace_in(src, "note", "foo", "bar");
    assert_eq!(out, "<note>bar</note><note>bar</note>");
}

// ─── edge cases ─────────────────────────────────────────────────────────────

#[test]
fn update_tag_with_only_whitespace_in_attrs_section() {
    // The section between name and `>` is just a space — exercises the
    // "all-whitespace remaining bytes" path in `merge_attrs`.
    let src = "<task >body</task>";
    let out = run_update(src, "task", &[("id", "1")]);
    assert_eq!(out, r#"<task id="1">body</task>"#);
}

#[test]
fn update_self_close_with_trailing_space_dedupes_separator() {
    // The section ends with whitespace, so a newly appended attr would
    // double-space — exercises the trim_start branch in merge_attrs.
    let src = "<task />";
    let out = run_update(src, "task", &[("id", "1")]);
    assert_eq!(out, r#"<task id="1"/>"#);
}

// ─── round-trip property: post-mutation doc still parses ──────────────────

#[test]
fn round_trip_update_remains_parseable() {
    let src = r#"<phase id="1"><task id="1.1"/><task id="1.2"/></phase>"#;
    let out = run_update(src, "task", &[("status", "done")]);
    parse(&out).expect("mutated doc must still parse");
}

#[test]
fn round_trip_replace_content_remains_parseable() {
    let src = "<task>old</task>";
    let out = run_replace_content(src, "task", "new <em>body</em>");
    let reparsed = parse(&out).expect("mutated doc must still parse");
    assert_eq!(reparsed.root_count(), 1);
}

#[test]
fn try_update_returns_error_on_invalid_attr_name() {
    let doc = parse("<task/>").unwrap();
    let sel = Selector::parse("task").unwrap();
    let err = doc.try_update(&sel, &[("1id", "x")]).unwrap_err();
    assert!(matches!(err, marxml::MutateError::InvalidAttrName { .. }));
}

#[test]
fn try_update_returns_error_on_duplicate_attr_name() {
    let doc = parse("<task/>").unwrap();
    let sel = Selector::parse("task").unwrap();
    let err = doc
        .try_update(&sel, &[("id", "a"), ("id", "b")])
        .unwrap_err();
    assert!(matches!(err, marxml::MutateError::DuplicateAttrName { .. }));
}

#[test]
fn replace_content_report_self_closing_skips() {
    // The selector matches a self-closing tag (no content range), so the
    // splice is skipped — but the report now surfaces that count instead
    // of silently zeroing it out.
    let doc = parse("<task/>").unwrap();
    let sel = Selector::parse("task").unwrap();
    let report = doc.replace_content_report(&sel, "X");
    assert_eq!(report.applied, 0);
    assert_eq!(report.skipped_self_closing, 1);
}

#[test]
fn replace_content_report_overlap_skips() {
    // Outer and inner `task` are both matched and both have replaceable
    // bodies; the inner splice overlaps the outer body and is recorded as
    // skipped.
    let doc = parse("<task>outer <task>inner</task></task>").unwrap();
    let sel = Selector::parse("task").unwrap();
    let report = doc.replace_content_report(&sel, "X");
    assert_eq!(report.applied, 1);
    assert_eq!(report.skipped_overlaps, 1);
}

#[test]
fn replace_text_escapes_replacement() {
    let doc = parse("<note>old</note>").unwrap();
    let sel = Selector::parse("note").unwrap();
    let out = doc.replace_text(&sel, "<script>");
    // The injected text is escaped, so reparsing finds no extra children.
    let reparsed = parse(&out).unwrap();
    assert_eq!(reparsed.root_count(), 1);
    let note = reparsed.root_elements().next().unwrap();
    assert_eq!(note.children().count(), 0);
    assert_eq!(note.content(), "&lt;script&gt;");
}

#[test]
fn replace_in_treats_dollar_as_literal() {
    // `$1` and `${name}` would, by default, be expanded by the `regex` crate
    // as capture-group references. The module documents replacements as
    // verbatim, so the literal `$1` must survive into the output.
    let src = "<task>price 100</task>";
    let out = run_replace_in(src, "task", r"(\d+)", "$1 USD");
    assert!(
        out.contains("$1 USD"),
        "replacement `$1 USD` should be literal, got {out:?}"
    );
}
