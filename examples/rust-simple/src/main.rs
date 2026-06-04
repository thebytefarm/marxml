//! rust-simple — parse, select, serialize.
//!
//! Reads samples/notes.md and prints the structured payload back as XML and
//! JSON. Read-only: never writes to disk.
//!
//! From the repo root:
//!
//! ```sh
//! cargo run -p example-simple
//! ```

use std::fs;
use std::path::PathBuf;

use marxml::{parse, Selector, SerializeOpts};

fn samples_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("samples")
        .join("notes.md")
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let src = fs::read_to_string(samples_path())?;
    let doc = parse(&src)?;

    // Compile the selector once. Reuse across queries.
    let ideas = Selector::parse(r#"note[tag="idea"]"#)?;

    println!("ideas ({}):", doc.select(&ideas).count());
    for n in doc.select(&ideas) {
        let id = n.attr("id").unwrap_or("?");
        let body: String = n.text().collect();
        println!("  {id}  {}", body.trim());
    }

    println!("\n-- to_xml (compact) --");
    println!("{}", doc.to_xml(&SerializeOpts::new()));

    println!("\n-- to_json --");
    println!("{}", doc.to_json());

    Ok(())
}
