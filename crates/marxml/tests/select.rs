//! Integration tests for `Selector` and `Markdown::select`.

#![allow(clippy::cast_possible_truncation)]

use marxml::{parse, Selector, SelectorError};
use rstest::rstest;

fn tags(doc: &marxml::Markdown, sel: &str) -> Vec<String> {
    let s = Selector::parse(sel).unwrap();
    doc.select(&s).map(|el| el.tag().to_string()).collect()
}

fn ids(doc: &marxml::Markdown, sel: &str) -> Vec<String> {
    let s = Selector::parse(sel).unwrap();
    doc.select(&s)
        .map(|el| el.attr("id").unwrap_or("").to_string())
        .collect()
}

// ─── Tag and universal ─────────────────────────────────────────────────────

#[test]
fn tag_selector_matches_by_name() {
    let doc = parse("<a/><b/><a/>").unwrap();
    assert_eq!(tags(&doc, "a"), vec!["a", "a"]);
}

#[test]
fn universal_matches_every_element() {
    let doc = parse("<a><b><c/></b></a>").unwrap();
    assert_eq!(tags(&doc, "*"), vec!["a", "b", "c"]);
}

#[test]
fn tag_selector_descends() {
    let doc = parse("<root><task/><task/></root>").unwrap();
    assert_eq!(tags(&doc, "task"), vec!["task", "task"]);
}

// ─── Attribute predicates ──────────────────────────────────────────────────

#[test]
fn attribute_presence() {
    let doc = parse(r#"<a x="1"/><a/><a y="2"/>"#).unwrap();
    assert_eq!(tags(&doc, "a[x]"), vec!["a"]);
    assert_eq!(tags(&doc, "a[y]"), vec!["a"]);
}

#[test]
fn attribute_equality() {
    let doc = parse(r#"<task id="1"/><task id="2"/>"#).unwrap();
    assert_eq!(ids(&doc, r#"task[id="2"]"#), vec!["2"]);
}

#[test]
fn attribute_starts_with() {
    let doc = parse(r#"<task id="4.1"/><task id="4.2"/><task id="5.1"/>"#).unwrap();
    assert_eq!(ids(&doc, r#"task[id^="4."]"#), vec!["4.1", "4.2"]);
}

#[test]
fn attribute_ends_with() {
    let doc = parse(r#"<task id="4.1"/><task id="5.1"/><task id="5.2"/>"#).unwrap();
    assert_eq!(ids(&doc, r#"task[id$=".1"]"#), vec!["4.1", "5.1"]);
}

#[test]
fn attribute_contains() {
    let doc = parse(r#"<task id="4.1"/><task id="5"/>"#).unwrap();
    assert_eq!(ids(&doc, r#"task[id*="."]"#), vec!["4.1"]);
}

#[test]
fn multiple_attribute_predicates_combined() {
    let doc = parse(r#"<task id="1" status="todo"/><task id="2" status="done"/>"#).unwrap();
    assert_eq!(ids(&doc, r#"task[id="1"][status="todo"]"#), vec!["1"]);
    assert_eq!(
        ids(&doc, r#"task[id="2"][status="todo"]"#),
        Vec::<String>::new()
    );
}

// ─── Combinators ───────────────────────────────────────────────────────────

#[test]
fn descendant_combinator() {
    let doc = parse("<phase><task><criterion/></task></phase>").unwrap();
    assert_eq!(tags(&doc, "phase criterion"), vec!["criterion"]);
}

#[test]
fn descendant_skips_intermediate_levels() {
    let doc = parse("<a><b><c><d/></c></b></a>").unwrap();
    assert_eq!(tags(&doc, "a d"), vec!["d"]);
}

#[test]
fn child_combinator_requires_direct_parent() {
    let doc = parse("<a><b><c/></b></a>").unwrap();
    assert_eq!(tags(&doc, "a > c"), Vec::<String>::new());
    assert_eq!(tags(&doc, "a > b"), vec!["b"]);
    assert_eq!(tags(&doc, "b > c"), vec!["c"]);
}

#[test]
fn chained_combinators_mix() {
    let doc = parse("<a><b><c><d/></c></b></a>").unwrap();
    assert_eq!(tags(&doc, "a b > c"), vec!["c"]);
    assert_eq!(tags(&doc, "a > b > c > d"), vec!["d"]);
}

#[test]
fn union_via_comma() {
    let doc = parse("<task/><phase/><note/>").unwrap();
    assert_eq!(tags(&doc, "task, phase"), vec!["task", "phase"]);
}

#[test]
fn union_dedupes_overlap() {
    // Both `task` and `*` match a `<task>` element; it should appear once.
    let doc = parse("<task/>").unwrap();
    assert_eq!(tags(&doc, "task, *"), vec!["task"]);
}

// ─── Pseudo-classes ────────────────────────────────────────────────────────

#[test]
fn first_child_pseudo() {
    let doc = parse("<root><a/><b/><c/></root>").unwrap();
    assert_eq!(tags(&doc, "root > *:first-child"), vec!["a"]);
}

#[test]
fn nth_child_pseudo_is_one_indexed() {
    let doc = parse("<root><a/><b/><c/></root>").unwrap();
    assert_eq!(tags(&doc, "root > *:nth-child(2)"), vec!["b"]);
    assert_eq!(tags(&doc, "root > *:nth-child(3)"), vec!["c"]);
}

#[test]
fn not_pseudo_excludes() {
    let doc = parse("<root><a/><b/><c/></root>").unwrap();
    assert_eq!(tags(&doc, "root > *:not(b)"), vec!["a", "c"]);
}

#[test]
fn not_pseudo_with_attribute() {
    let doc = parse(r#"<task id="1"/><task/>"#).unwrap();
    assert_eq!(tags(&doc, "task:not([id])"), vec!["task"]);
}

// ─── Sub-select from an element ────────────────────────────────────────────

#[test]
fn elementref_select_scopes_to_descendants() {
    let doc = parse("<phase id=\"1\"><task/></phase><phase id=\"2\"><task/></phase>").unwrap();
    let phase1_sel = Selector::parse(r#"phase[id="1"]"#).unwrap();
    let task_sel = Selector::parse("task").unwrap();
    let phase1 = doc.select(&phase1_sel).next().unwrap();
    let tasks: Vec<_> = phase1.select(&task_sel).collect();
    assert_eq!(tasks.len(), 1);
}

// ─── ElementRef::text ──────────────────────────────────────────────────────

#[test]
fn text_segments_skip_child_markup() {
    let doc = parse("<task>do <em>this</em> now</task>").unwrap();
    let sel = Selector::parse("task").unwrap();
    let task = doc.select(&sel).next().unwrap();
    let segments: Vec<_> = task.text().collect();
    assert_eq!(segments, vec!["do ", " now"]);
}

#[test]
fn text_segments_for_pure_text() {
    let doc = parse("<note>only text</note>").unwrap();
    let sel = Selector::parse("note").unwrap();
    let note = doc.select(&sel).next().unwrap();
    let segments: Vec<_> = note.text().collect();
    assert_eq!(segments, vec!["only text"]);
}

#[test]
fn text_segments_empty_for_self_close() {
    let doc = parse("<spacer/>").unwrap();
    let sel = Selector::parse("spacer").unwrap();
    let spacer = doc.select(&sel).next().unwrap();
    assert!(spacer.text().next().is_none());
}

#[test]
fn text_segments_when_only_children() {
    let doc = parse("<wrap><a/><b/></wrap>").unwrap();
    let sel = Selector::parse("wrap").unwrap();
    let wrap = doc.select(&sel).next().unwrap();
    assert!(wrap.text().next().is_none());
}

// ─── Selector grammar errors ───────────────────────────────────────────────

#[test]
fn empty_selector_is_an_error() {
    let err = Selector::parse("").unwrap_err();
    assert!(matches!(err, SelectorError::Empty));
}

#[test]
fn whitespace_only_selector_is_an_error() {
    let err = Selector::parse("   ").unwrap_err();
    assert!(matches!(err, SelectorError::Empty));
}

#[rstest]
#[case("a[", "expected attribute name")]
#[case("a[x", "expected attribute operator or ']'")]
#[case("a[x=]", "'\"'")]
#[case("a:nth-child", "'(' after :nth-child")]
#[case("a:nth-child()", "expected digit")]
#[case("a:nth-child(2", "')' after nth-child argument")]
#[case("a:bogus", "unsupported pseudo-class")]
#[case("a:not", "'(' after :not")]
#[case("a:not(b", "')' after :not argument")]
#[case("a:", "expected pseudo-class name")]
#[case("a,", "unexpected end")]
#[case("a b ,", "unexpected end")]
#[case("a b @ c", "expected tag name, '*', or predicate")]
#[case("a[x@", "expected attribute operator or ']'")]
fn malformed_selector_errors(#[case] sel: &str, #[case] fragment: &str) {
    let err = Selector::parse(sel).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains(fragment),
        "selector {sel:?}: expected error containing {fragment:?}, got {msg:?}"
    );
}

#[test]
fn unexpected_end_in_attribute_value() {
    let err = Selector::parse(r#"a[x="unterminated"#).unwrap_err();
    assert!(matches!(err, SelectorError::UnexpectedEnd));
}

#[test]
fn no_match_when_required_ancestor_missing() {
    // Exercises the descendant-combinator "no ancestor found" path.
    let doc = parse("<a/>").unwrap();
    let sel = Selector::parse("b a").unwrap();
    assert_eq!(doc.select(&sel).count(), 0);
}

#[test]
fn missing_combinator_after_tag_errors() {
    // `a@b` — no whitespace, no `>`, not `,` — exercises the
    // "expected combinator or ','" branch.
    let err = Selector::parse("a@b").unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("expected combinator or ','"), "got {msg:?}");
}

#[test]
fn tag_less_attribute_selector_matches_any_element() {
    let doc = parse(r#"<task id="1"/><phase id="2"/><note/>"#).unwrap();
    let sel = Selector::parse("[id]").unwrap();
    let matches: Vec<_> = doc.select(&sel).map(|el| el.tag().to_string()).collect();
    assert_eq!(matches, vec!["task", "phase"]);
}

#[test]
fn selector_value_decodes_entity_references() {
    // The attribute on the doc parses as the literal value `a&b`. The
    // selector value also decodes entities, so `[id="a&amp;b"]` finds it.
    let doc = parse(r#"<x id="a&amp;b"/>"#).unwrap();
    let sel = Selector::parse(r#"x[id="a&amp;b"]"#).unwrap();
    assert_eq!(doc.select(&sel).count(), 1);
}

#[test]
fn nth_child_zero_is_a_parse_error() {
    // `:nth-child(0)` could never match a 1-indexed sibling position, so the
    // parser rejects it at compile time rather than silently producing an
    // unmatchable selector.
    let err = Selector::parse("root > *:nth-child(0)").unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("nth-child") && msg.contains("1 or greater"),
        "expected nth-child(0) rejection, got {msg:?}"
    );
}

#[test]
fn selector_displays_as_debug() {
    // Ensures Selector implements Debug for diagnostics.
    let sel = Selector::parse("a > b").unwrap();
    assert!(!format!("{sel:?}").is_empty());
}

#[test]
fn selector_with_underscored_tags() {
    let doc = parse("<my_thing/>").unwrap();
    assert_eq!(tags(&doc, "my_thing"), vec!["my_thing"]);
}

#[test]
fn selector_with_hyphenated_tags() {
    let doc = parse("<my-thing/>").unwrap();
    assert_eq!(tags(&doc, "my-thing"), vec!["my-thing"]);
}

#[test]
fn selector_with_dotted_attribute_value() {
    let doc = parse(r#"<task id="4.1"/>"#).unwrap();
    assert_eq!(ids(&doc, r#"task[id="4.1"]"#), vec!["4.1"]);
}
