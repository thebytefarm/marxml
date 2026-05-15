//! Fixture-driven integration tests.
//!
//! Each directory under `tests/fixtures/` holds inputs that exercise a
//! different code path:
//!
//! - `parse/` — must parse cleanly; we snapshot the element tree.
//! - `parse_fail/` — must return `Err`; we snapshot the error message.
//! - `select/` — must parse cleanly; for each file we run a small selector
//!   probe set and snapshot the matches.
//! - `mutate/` — must parse cleanly; we exercise update / `replace_*` and
//!   snapshot each output.
//! - `serialize/` — must parse cleanly; we snapshot `to_xml` (default +
//!   pretty) and `to_json`.
//! - `validate/` — must parse cleanly; we run `validate()` against a
//!   hard-coded gg-style schema and snapshot the report.
//!
//! Snapshots live in `tests/snapshots/` (auto-managed by `insta`). Re-run
//! with `INSTA_UPDATE=auto` to regenerate after intentional changes.

#![allow(clippy::needless_pass_by_value)]

use std::fmt::Write as _;

use marxml::{parse, validate, ElementRef, Markdown, ParseError, Schema, Selector, SerializeOpts};

// ─── parse/ ────────────────────────────────────────────────────────────────

#[test]
fn fixture_parse_success() {
    insta::glob!("fixtures/parse/*.md", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let doc = parse(&input).unwrap_or_else(|e| panic!("{}: {}", path.display(), e));
        insta::assert_snapshot!(dump_doc(&doc));
    });
}

// ─── parse_fail/ ───────────────────────────────────────────────────────────

#[test]
fn fixture_parse_failure() {
    insta::glob!("fixtures/parse_fail/*.md", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let err = parse(&input).expect_err(&format!("{} should fail", path.display()));
        insta::assert_snapshot!(format!("{}: {}", error_kind(&err), err));
    });
}

// ─── select/ ───────────────────────────────────────────────────────────────

/// Selectors applied to every fixture in `select/`. The output snapshots the
/// matching tag list for each.
const PROBE_SELECTORS: &[&str] = &[
    "task",
    "phase",
    "*",
    r#"task[status="todo"]"#,
    r#"task[id^="1."]"#,
    "phase task",
    "phase > task",
    "*:first-child",
    "*:not([id])",
];

#[test]
fn fixture_select_probes() {
    insta::glob!("fixtures/select/*.md", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let doc = parse(&input).expect("clean input");
        let mut out = String::new();
        for sel_src in PROBE_SELECTORS {
            let Ok(sel) = Selector::parse(sel_src) else {
                writeln!(out, "{sel_src} -> <invalid selector>").unwrap();
                continue;
            };
            let matched: Vec<String> = doc.select(&sel).map(|el| format_brief(&el)).collect();
            if matched.is_empty() {
                writeln!(out, "{sel_src} -> []").unwrap();
            } else {
                writeln!(out, "{sel_src} -> [{}]", matched.join(", ")).unwrap();
            }
        }
        insta::assert_snapshot!(out);
    });
}

// ─── mutate/ ───────────────────────────────────────────────────────────────

#[test]
fn fixture_mutate_probes() {
    insta::glob!("fixtures/mutate/*.md", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let doc = parse(&input).expect("clean input");
        let task = Selector::parse("task").unwrap();
        let mut out = String::new();

        out.push_str("--- update(task, [(status, done)]) ---\n");
        out.push_str(&doc.update(&task, &[("status", "done")]));
        out.push_str("\n\n--- replace_content(task, NEW) ---\n");
        out.push_str(&doc.replace_content(&task, "NEW"));
        out.push_str("\n\n--- replace_in(task, /todo/, done) ---\n");
        let re = regex::Regex::new("todo").unwrap();
        out.push_str(&doc.replace_in(&task, &re, "done"));
        out.push('\n');

        insta::assert_snapshot!(out);
    });
}

// ─── serialize/ ────────────────────────────────────────────────────────────

#[test]
fn fixture_serialize_probes() {
    insta::glob!("fixtures/serialize/*.md", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let doc = parse(&input).expect("clean input");
        let mut out = String::new();
        out.push_str("--- to_xml(default) ---\n");
        out.push_str(&doc.to_xml(&SerializeOpts::default()));
        out.push_str("\n\n--- to_xml(pretty) ---\n");
        out.push_str(&doc.to_xml(&SerializeOpts::pretty()));
        out.push_str("\n\n--- to_json ---\n");
        let json = doc.to_json();
        out.push_str(&serde_json::to_string_pretty(&json).unwrap());
        out.push('\n');
        insta::assert_snapshot!(out);
    });
}

// ─── validate/ ─────────────────────────────────────────────────────────────

#[test]
fn fixture_validate_probes() {
    let schema = build_gg_schema();
    insta::glob!("fixtures/validate/*.md", |path| {
        let input = std::fs::read_to_string(path).unwrap();
        let doc = parse(&input).expect("clean input");
        let report = validate(&doc, &schema);
        let mut out = String::new();
        writeln!(out, "valid: {}", report.is_valid()).unwrap();
        for err in report.errors() {
            writeln!(out, "  - {err}").unwrap();
        }
        insta::assert_snapshot!(out);
    });
}

// ─── helpers ───────────────────────────────────────────────────────────────

fn dump_doc(doc: &Markdown) -> String {
    let mut out = String::new();
    writeln!(out, "root_count = {}", doc.root_count()).unwrap();
    for (i, el) in doc.root_elements().enumerate() {
        writeln!(out, "root[{i}]:").unwrap();
        dump_node(&el, 1, &mut out);
    }
    out
}

fn dump_node(el: &ElementRef<'_>, depth: usize, out: &mut String) {
    let indent = "  ".repeat(depth);
    let loc = el.location();
    write!(out, "{indent}<{}", el.tag()).unwrap();
    for (k, v) in el.attrs() {
        write!(out, " {k}={v:?}").unwrap();
    }
    if el.is_self_closing() {
        write!(out, " />").unwrap();
    } else {
        write!(out, ">").unwrap();
    }
    writeln!(
        out,
        "  @ L{}:O{} -> L{}:O{}",
        loc.start.line, loc.start.offset, loc.end.line, loc.end.offset
    )
    .unwrap();
    let content = el.content();
    let text: String = el.text().collect();
    if !content.is_empty() && el.children().next().is_none() {
        writeln!(out, "{indent}  content={content:?}").unwrap();
    } else if !text.is_empty() {
        writeln!(out, "{indent}  text={text:?}").unwrap();
    }
    for child in el.children() {
        dump_node(&child, depth + 1, out);
    }
}

fn format_brief(el: &ElementRef<'_>) -> String {
    if let Some(id) = el.attr("id") {
        format!("{}#{id}", el.tag())
    } else {
        el.tag().to_string()
    }
}

fn error_kind(e: &ParseError) -> &'static str {
    match e {
        ParseError::UnclosedTag { .. } => "UnclosedTag",
        ParseError::MismatchedClose { .. } => "MismatchedClose",
        ParseError::StrayClose { .. } => "StrayClose",
        ParseError::MalformedTag { .. } => "MalformedTag",
        ParseError::MalformedAttribute { .. } => "MalformedAttribute",
        ParseError::DuplicateId { .. } => "DuplicateId",
    }
}

fn build_gg_schema() -> Schema {
    use marxml::schema::AttrKind;
    Schema::builder()
        .tag("phase", |t| {
            t.attr("id", AttrKind::String.required())
                .attr(
                    "status",
                    AttrKind::Enum(vec!["todo".into(), "done".into()]).required(),
                )
                .attr("name", AttrKind::String)
        })
        .tag("task", |t| {
            t.attr("id", AttrKind::String.required())
                .attr(
                    "status",
                    AttrKind::Enum(vec!["todo".into(), "done".into(), "skip".into()]),
                )
                .child_optional("status")
                .child_optional("criterion")
                .child_optional("files")
        })
        .tag("status", marxml::schema::TagBuilder::content_required)
        .build()
}
