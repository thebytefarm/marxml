//! rust-advanced — selector reuse, mutation, validation, pretty serialize.
//!
//! Reads samples/plan.md (never modified) and writes the rewritten document
//! to out/plan.xml. Run `../reset.sh` to clear out/.
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
    // the original doc is unchanged.
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

    // (4) Pretty XML written to out/.
    let out_dir = root.join("out");
    fs::create_dir_all(&out_dir)?;
    let xml = doc3.to_xml(&SerializeOpts::pretty());
    let out_path = out_dir.join("plan.xml");
    fs::write(&out_path, &xml)?;
    println!("\nwrote {} ({} bytes)", out_path.display(), xml.len());

    println!("\n-- preview --\n{xml}");

    Ok(())
}
