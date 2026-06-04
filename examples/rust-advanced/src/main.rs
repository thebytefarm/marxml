//! rust-advanced — selector reuse, mutation, validation, two-shape serialize.
//!
//! Reads samples/plan.md (a real markdown document with embedded XML),
//! mutates it surgically, and writes two outputs:
//!
//! - `out/plan.md`   — full markdown document with edits applied. Headings,
//!   bullets, and prose are byte-preserved; only the inside of `<task>` /
//!   `<phase>` tags changes. Still renders cleanly on GitHub.
//! - `out/plan.xml`  — structured payload, via `to_xml(structured())`. Single
//!   `<markdown>` root, indented, markdown noise stripped between siblings.
//! - `out/plan.json` — structured tree, via `to_json` + pretty-printed.
//! - `out/plan.yaml` — same structured tree, via `to_yaml`. Same shape as
//!   `to_json`, just YAML-encoded.
//!
//! From the repo root:
//!
//! ```sh
//! cargo run -p example-advanced
//! ```

use std::fs;
use std::path::PathBuf;

use marxml::{parse, schema::AttrKind, validate, Schema, Selector, SerializeOpts};

fn here() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = here();
    let src = fs::read_to_string(root.join("samples").join("plan.md"))?;

    let doc = parse(&src)?;

    let pending = Selector::parse(r#"task[status="todo"]"#)?;
    let everything = Selector::parse("task")?;
    println!("pending: {}", doc.select(&pending).count());
    println!("total:   {}", doc.select(&everything).count());

    // (1) Mark every "todo" task as "done". `update` returns a new String —
    // markdown prose, headings, and bullets come back unchanged.
    let after_update = doc.update(&pending, &[("status", "done")]);

    // Mutators return Strings; re-parse to chain further mutations.
    let doc2 = parse(&after_update)?;

    // (2) Swap one task's body. `replace_text` escapes `<`, `&`, `"` so the
    // splice can't break out of the parent tag.
    let only_first = Selector::parse(r#"task[id="1.1"]"#)?;
    let after_swap = doc2.replace_text(&only_first, "design + document the schema DSL (priority)");
    let doc3 = parse(&after_swap)?;

    // (3) Validate the rewritten doc.
    let schema = Schema::builder()
        .tag("task", |t| {
            t.attr("id", AttrKind::String.required())
                .attr("status", AttrKind::one_of(["todo", "done"]).required())
        })
        .tag("phase", |t| {
            t.attr("id", AttrKind::String.required()).attr(
                "status",
                AttrKind::one_of(["todo", "in-progress", "done"]).required(),
            )
        })
        .build();

    let report = validate(&doc3, &schema);
    if report.is_valid() {
        println!("validation: OK");
    } else {
        println!("validation: {} error(s)", report.len());
        for e in &report {
            println!("  - {e}");
        }
    }

    let out_dir = root.join("out");
    fs::create_dir_all(&out_dir)?;

    // (4a) Full markdown document with surgical edits. `raw()` is the source
    // string the latest re-parse was built from — i.e. the post-mutation
    // markdown, with every byte outside the touched tags identical to the
    // input.
    let md_path = out_dir.join("plan.md");
    fs::write(&md_path, doc3.raw())?;
    println!(
        "\nwrote {} ({} bytes) — full markdown with edits",
        md_path.display(),
        doc3.raw().len()
    );

    // (4b) Structured payload as a single-root, valid XML document.
    // `structured()` wraps in `<markdown>` and drops the markdown noise
    // between sibling tags.
    let xml = doc3.to_xml(&SerializeOpts::structured());
    let xml_path = out_dir.join("plan.xml");
    fs::write(&xml_path, &xml)?;
    println!(
        "wrote {} ({} bytes) — structured XML extract",
        xml_path.display(),
        xml.len()
    );

    // (4c) Same structured tree, encoded as JSON.
    // `to_json_with(structured())` wraps under a `markdown` key and empties
    // non-leaf `text` fields so the JSON matches the XML output in shape.
    let json = serde_json::to_string_pretty(&doc3.to_json_with(&SerializeOpts::structured()))?;
    let json_path = out_dir.join("plan.json");
    fs::write(&json_path, &json)?;
    println!(
        "wrote {} ({} bytes) — structured JSON extract",
        json_path.display(),
        json.len()
    );

    // (4d) Same structured tree, encoded as YAML.
    let yaml = doc3.to_yaml_with(&SerializeOpts::structured());
    let yaml_path = out_dir.join("plan.yaml");
    fs::write(&yaml_path, &yaml)?;
    println!(
        "wrote {} ({} bytes) — structured YAML extract",
        yaml_path.display(),
        yaml.len()
    );

    println!("\n-- markdown preview (out/plan.md) --\n{}", doc3.raw());
    println!("\n-- xml preview (out/plan.xml) --\n{xml}");
    println!("\n-- json preview (out/plan.json) --\n{json}");
    println!("\n-- yaml preview (out/plan.yaml) --\n{yaml}");

    Ok(())
}
