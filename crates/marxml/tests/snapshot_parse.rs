//! Snapshot tests for the parsed structure of representative documents.
//!
//! Snapshots live under `tests/snapshots/`. Review diffs on PR.

use std::fmt::Write as _;

use marxml::{ElementRef, Markdown};

fn dump(doc: &Markdown) -> String {
    let mut out = String::new();
    for root in doc.root_elements() {
        dump_node(&root, 0, &mut out);
    }
    out
}

fn dump_node(node: &ElementRef<'_>, depth: usize, out: &mut String) {
    for _ in 0..depth {
        out.push_str("  ");
    }
    out.push('<');
    out.push_str(node.tag());
    for (k, v) in node.attrs() {
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        out.push_str(v);
        out.push('"');
    }
    if node.is_self_closing() {
        out.push_str("/> ");
    } else {
        out.push('>');
        out.push(' ');
        let content = node.content();
        if !content.is_empty() {
            write!(out, "body={content:?} ").unwrap();
        }
    }
    let loc = node.location();
    write!(
        out,
        "@ {}:{} → {}:{}",
        loc.start.line, loc.start.offset, loc.end.line, loc.end.offset
    )
    .unwrap();
    out.push('\n');
    for child in node.children() {
        dump_node(&child, depth + 1, out);
    }
}

#[test]
fn snapshot_gg_style_doc() {
    let src = r#"# Project

<phase id="1" status="todo">

<task id="1.1" status="todo">
<criterion>do thing one</criterion>
<criterion>do thing two</criterion>
</task>

<task id="1.2" status="done">
<criterion>done already</criterion>
</task>

</phase>

<phase id="2" status="todo">
<task id="2.1"/>
</phase>
"#;

    let doc = marxml::parse(src).expect("gg-style doc must parse cleanly");
    insta::assert_snapshot!("gg_style_doc", dump(&doc));
}

#[test]
fn snapshot_mixed_text_and_tags() {
    let src =
        "Intro paragraph.\n\n<note>quick</note>\n\nMid text with x < 3.\n\n<note>second</note>\n";
    let doc = marxml::parse(src).expect("mixed doc must parse");
    insta::assert_snapshot!("mixed_text_and_tags", dump(&doc));
}
