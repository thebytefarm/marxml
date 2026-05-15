//! Parse + select + serialize benches.
//!
//! Run locally with `cargo bench`. In CI, `CodSpeed`'s GitHub Action wraps
//! this same binary via the `codspeed-criterion-compat` shim and produces
//! instruction-count measurements that don't depend on runner load.

#![allow(missing_docs)]

use std::fmt::Write as _;

use codspeed_criterion_compat::{
    black_box, criterion_group, criterion_main, Criterion, Throughput,
};

fn corpus_small() -> String {
    r#"<phase id="1" status="todo">
<task id="1.1"><status>todo</status></task>
<task id="1.2"><status>done</status></task>
</phase>"#
        .to_string()
}

fn corpus_medium() -> String {
    let mut s = String::with_capacity(64 * 1024);
    s.push_str("# Mixed doc\n\nIntro paragraph.\n\n");
    for i in 0..200 {
        writeln!(
            s,
            r#"<phase id="{i}"><task id="{i}.1" status="todo"><status>todo</status></task><task id="{i}.2" status="done"><status>done</status></task></phase>"#,
        )
        .unwrap();
    }
    s
}

fn corpus_large() -> String {
    let mut s = String::with_capacity(1_000_000);
    for i in 0..5000 {
        let state = if i % 2 == 0 { "todo" } else { "done" };
        writeln!(
            s,
            r#"<task id="{i}" status="{state}"><status>{state}</status></task>"#,
        )
        .unwrap();
    }
    s
}

fn bench_parse(c: &mut Criterion) {
    let mut g = c.benchmark_group("parse");
    for (label, src) in [
        ("small_~200B", corpus_small()),
        ("medium_~50KB", corpus_medium()),
        ("large_~500KB", corpus_large()),
    ] {
        g.throughput(Throughput::Bytes(src.len() as u64));
        g.bench_function(label, |b| {
            b.iter(|| {
                let doc = marxml::parse(black_box(&src)).expect("clean corpus");
                black_box(doc);
            });
        });
    }
    g.finish();
}

fn bench_select(c: &mut Criterion) {
    let src = corpus_medium();
    let doc = marxml::parse(&src).expect("clean corpus");
    let sel_tag = marxml::Selector::parse("task").unwrap();
    let sel_attr = marxml::Selector::parse(r#"task[status="todo"]"#).unwrap();
    let sel_deep = marxml::Selector::parse("phase task").unwrap();

    let mut g = c.benchmark_group("select");
    g.bench_function("by_tag", |b| {
        b.iter(|| black_box(doc.select(&sel_tag).count()));
    });
    g.bench_function("by_attr_eq", |b| {
        b.iter(|| black_box(doc.select(&sel_attr).count()));
    });
    g.bench_function("descendant", |b| {
        b.iter(|| black_box(doc.select(&sel_deep).count()));
    });
    g.finish();
}

fn bench_serialize(c: &mut Criterion) {
    let src = corpus_medium();
    let doc = marxml::parse(&src).expect("clean corpus");
    let mut g = c.benchmark_group("serialize");
    g.bench_function("to_xml_tight", |b| {
        b.iter(|| black_box(doc.to_xml(&marxml::SerializeOpts::default())));
    });
    g.bench_function("to_xml_pretty", |b| {
        b.iter(|| black_box(doc.to_xml(&marxml::SerializeOpts::pretty())));
    });
    g.finish();
}

fn bench_mutate(c: &mut Criterion) {
    let src = corpus_medium();
    let doc = marxml::parse(&src).expect("clean corpus");
    let sel = marxml::Selector::parse(r#"task[status="todo"]"#).unwrap();
    let mut g = c.benchmark_group("mutate");
    g.bench_function("update_attr_all_matches", |b| {
        b.iter(|| black_box(doc.update(&sel, &[("status", "done")])));
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_parse,
    bench_select,
    bench_serialize,
    bench_mutate
);
criterion_main!(benches);
