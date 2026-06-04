//! Integration tests for serialization (`to_xml`, `to_json`, `Display`).

use marxml::{parse, SerializeOpts};

// ─── Display impls ─────────────────────────────────────────────────────────

#[test]
fn markdown_display_returns_raw() {
    let src = "Intro\n\n<note>hi</note>\n\nTail";
    let doc = parse(src).unwrap();
    assert_eq!(format!("{doc}"), src);
}

#[test]
fn element_display_returns_outer_xml_from_source() {
    let src = r#"prelude <task id="1">body</task> postlude"#;
    let doc = parse(src).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(format!("{el}"), r#"<task id="1">body</task>"#);
}

#[test]
fn element_display_for_self_close() {
    let src = "x <spacer/> y";
    let doc = parse(src).unwrap();
    let el = doc.root_elements().next().unwrap();
    assert_eq!(format!("{el}"), "<spacer/>");
}

// ─── to_xml ────────────────────────────────────────────────────────────────

#[test]
fn to_xml_default_is_tight_single_string() {
    let src = "before <task id=\"1\">body</task> after";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::default());
    assert_eq!(out, "<task id=\"1\">body</task>");
}

#[test]
fn to_xml_concatenates_multiple_roots_preserving_self_close() {
    let src = "<a/><b/><c/>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::default());
    // Source used self-close; serializer preserves that style.
    assert_eq!(out, "<a/><b/><c/>");
}

#[test]
fn to_xml_open_close_pairs_stay_open_close_by_default() {
    let src = "<a></a><b></b>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::default());
    assert_eq!(out, "<a></a><b></b>");
}

#[test]
fn to_xml_self_close_empty_opt() {
    let src = "<a></a>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::default().self_close_empty());
    assert_eq!(out, "<a/>");
}

#[test]
fn to_xml_preserves_attrs() {
    let src = r#"<task id="1" status="todo">body</task>"#;
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::default());
    assert_eq!(out, r#"<task id="1" status="todo">body</task>"#);
}

#[test]
fn to_xml_pretty_indents_children() {
    let src = "<root><a/><b/></root>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::pretty());
    let expected = "<root>\n  <a/>\n  <b/>\n</root>";
    assert_eq!(out, expected);
}

#[test]
fn to_xml_pretty_separates_multiple_roots_with_newlines() {
    let src = "<a/><b/>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::pretty());
    assert_eq!(out, "<a/>\n<b/>");
}

#[test]
fn to_xml_pretty_nested_indents_increment() {
    let src = "<a><b><c/></b></a>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::pretty());
    let expected = "<a>\n  <b>\n    <c/>\n  </b>\n</a>";
    assert_eq!(out, expected);
}

#[test]
fn to_xml_pretty_does_not_indent_mixed_content() {
    // `<p>a <b/> c</p>` is mixed content. Pretty mode must not inject
    // indentation in front of the inline child, because that would change
    // the parent's text stream (`a   <b/> c` instead of `a <b/> c`).
    let src = "<p>a <b/> c</p>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::pretty());
    assert_eq!(out, "<p>a <b/> c</p>");
}

#[test]
fn to_xml_pretty_preserves_existing_self_close() {
    let src = "<a/>";
    let doc = parse(src).unwrap();
    // self_close_empty is true via `pretty()`.
    let out = doc.to_xml(&SerializeOpts::pretty());
    assert_eq!(out, "<a/>");
}

#[test]
fn to_xml_round_trips_through_parse() {
    let src = r#"<phase id="1"><task id="1.1"/><task id="1.2"/></phase>"#;
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::default());
    parse(&out).expect("to_xml output must parse cleanly");
}

#[test]
fn to_xml_no_elements_yields_empty() {
    let src = "just markdown, no elements";
    let doc = parse(src).unwrap();
    assert_eq!(doc.to_xml(&SerializeOpts::default()), "");
}

// ─── to_json ───────────────────────────────────────────────────────────────

#[test]
fn to_json_shapes_simple_element() {
    let src = r#"<task id="1">body</task>"#;
    let doc = parse(src).unwrap();
    let val = doc.to_json();
    let array = val.as_array().unwrap();
    assert_eq!(array.len(), 1);
    let el = &array[0];
    assert_eq!(el["tag"], "task");
    assert_eq!(el["attrs"]["id"], "1");
    assert_eq!(el["text"], "body");
    assert_eq!(el["children"].as_array().unwrap().len(), 0);
    assert_eq!(el["selfClosing"], false);
}

#[test]
fn to_json_text_excludes_comments_and_includes_cdata_content() {
    // Comments disappear; CDATA inner content survives as literal text.
    let src = "<note>hi<!--ignore-->there<![CDATA[<x/>]]>!</note>";
    let doc = parse(src).unwrap();
    let val = doc.to_json();
    let el = &val.as_array().unwrap()[0];
    assert_eq!(el["text"], "hithere<x/>!");
}

#[test]
fn to_xml_escapes_loose_lt_in_text_body() {
    // Source has a literal `<` (not followed by a name-start) that the
    // permissive tokenizer accepts as prose. `to_xml` must escape it on
    // emission so downstream strict XML parsers can't resync on `</task>`
    // hidden inside the body.
    let doc = parse("<task>x < 3</task>").unwrap();
    let out = doc.to_xml(&SerializeOpts::default());
    assert!(
        !out.contains("x < 3"),
        "body should be escaped, got {out:?}"
    );
    assert!(out.contains("&lt;"), "expected &lt; entity, got {out:?}");
}

#[test]
fn entity_round_trip_does_not_double_escape() {
    // `&amp;` in the source must NOT become `&amp;amp;` after round-tripping
    // through `to_xml` — the tokenizer decodes entities once and the
    // serializer re-escapes once, producing the original byte form.
    let src = r#"<task name="A &amp; B"/>"#;
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::default());
    assert_eq!(out, src);
}

#[test]
fn to_json_text_is_child_stripped() {
    // The `text` field carries only direct text segments of an element —
    // child-element markup does not appear in it, so deeply nested documents
    // do not multiply allocations.
    let src = r"<task>pre <child/> post</task>";
    let doc = parse(src).unwrap();
    let val = doc.to_json();
    let el = &val.as_array().unwrap()[0];
    assert_eq!(el["text"], "pre  post");
    assert_eq!(el["children"].as_array().unwrap().len(), 1);
}

#[test]
fn to_json_includes_location() {
    let src = "x\n<task/>";
    let doc = parse(src).unwrap();
    let val = doc.to_json();
    let el = &val.as_array().unwrap()[0];
    assert_eq!(el["location"]["start"]["line"], 2);
    assert_eq!(el["location"]["start"]["offset"], 2);
}

#[test]
fn to_json_nests_children() {
    let src = "<phase><task/></phase>";
    let doc = parse(src).unwrap();
    let val = doc.to_json();
    let phase = &val.as_array().unwrap()[0];
    let kids = phase["children"].as_array().unwrap();
    assert_eq!(kids.len(), 1);
    assert_eq!(kids[0]["tag"], "task");
    assert_eq!(kids[0]["selfClosing"], true);
}

#[test]
fn to_json_empty_doc() {
    let doc = parse("plain text").unwrap();
    let val = doc.to_json();
    assert_eq!(val.as_array().unwrap().len(), 0);
}

#[test]
fn to_json_is_stable_for_serde_round_trip() {
    let src = r#"<task id="1"/>"#;
    let doc = parse(src).unwrap();
    let val = doc.to_json();
    let s = serde_json::to_string(&val).unwrap();
    let reparsed: serde_json::Value = serde_json::from_str(&s).unwrap();
    assert_eq!(reparsed, val);
}

// ─── strip_text + wrap_in (structured XML) ─────────────────────────────────

#[test]
fn to_xml_strip_text_drops_inter_element_prose() {
    // `<phase>` has markdown prose between its `<task>` children. Without
    // strip_text the emit falls back to byte-preserving tight mode for the
    // mixed body; with strip_text=true the children are emitted on their
    // own indented lines and the prose is dropped.
    let src = "<phase>\n  noise\n  <task id=\"1\"/>\n  more noise\n  <task id=\"2\"/>\n</phase>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::pretty().strip_text(true));
    let expected = "<phase>\n  <task id=\"1\"/>\n  <task id=\"2\"/>\n</phase>";
    assert_eq!(out, expected);
}

#[test]
fn to_xml_strip_text_preserves_leaf_text_body() {
    // A leaf element's text body is content, not inter-element prose.
    // strip_text must keep it.
    let src = "<task>body text</task>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::pretty().strip_text(true));
    assert_eq!(out, "<task>body text</task>");
}

#[test]
fn to_xml_with_root_wraps_multi_root_output() {
    let src = "<a/><b/>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::pretty().with_root("doc"));
    // Children sit one level deep inside the synthetic root, so they take
    // one indent step.
    let expected = "<doc>\n  <a/>\n  <b/>\n</doc>";
    assert_eq!(out, expected);
}

#[test]
fn to_xml_structured_constructor_combines_both() {
    // The convenience constructor sets indent + self_close_empty +
    // strip_text + wrap_in("markdown") in one call.
    let opts = SerializeOpts::structured();
    assert_eq!(opts.indent.as_deref(), Some("  "));
    assert!(opts.self_close_empty);
    assert!(opts.strip_text);
    assert_eq!(opts.wrap_in.as_deref(), Some("markdown"));
}

#[test]
fn to_xml_structured_produces_single_root_document() {
    // The combined output is a well-formed XML document: single `<markdown>`
    // root, indented children, no markdown noise between siblings.
    let src = "## Heading\n\n<phase>\n  prose\n  <task id=\"1\"/>\n  <task id=\"2\"/>\n</phase>\n\nmore prose\n\n<phase>\n  <task id=\"3\"/>\n</phase>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::structured());
    // Single wrapping root.
    assert!(out.starts_with("<markdown>"));
    assert!(out.ends_with("</markdown>"));
    // Inter-element prose dropped.
    assert!(!out.contains("prose"));
    // Children survive.
    assert!(out.contains("<task id=\"1\"/>"));
    assert!(out.contains("<task id=\"3\"/>"));
}

#[test]
fn to_xml_structured_overrides_root_name() {
    let src = "<a/>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::structured().with_root("plan"));
    assert!(out.starts_with("<plan>"));
    assert!(out.ends_with("</plan>"));
}

#[test]
fn to_xml_structured_round_trips_through_parse() {
    let src = "<phase>\n  noise\n  <task id=\"1\"/>\n  <task id=\"2\"/>\n</phase>";
    let doc = parse(src).unwrap();
    let out = doc.to_xml(&SerializeOpts::structured());
    let reparsed = parse(&out).expect("structured output must parse cleanly");
    assert_eq!(reparsed.root_count(), 1); // the synthetic <markdown> wrapper
}
