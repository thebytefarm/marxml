//! Integration tests for the validation API.

use marxml::schema::AttrKind;
use marxml::{parse, validate, Schema, ValidationError};

fn task_schema() -> Schema {
    Schema::builder()
        .tag("task", |t| {
            t.attr("id", AttrKind::String.required())
                .attr("status", AttrKind::Enum(vec!["todo".into(), "done".into()]))
                .child_required("status")
                .content_required()
        })
        .build()
}

// ─── Happy path ─────────────────────────────────────────────────────────────

#[test]
fn fully_compliant_doc_validates() {
    let src = r#"<task id="1" status="todo"><status>todo</status></task>"#;
    let doc = parse(src).unwrap();
    let report = validate(&doc, &task_schema());
    assert!(report.is_valid());
    assert!(report.errors().is_empty());
}

#[test]
fn unspecified_tags_are_ignored() {
    let src = "<unscoped/>";
    let doc = parse(src).unwrap();
    let report = validate(&doc, &task_schema());
    assert!(report.is_valid());
}

// ─── MissingAttr ────────────────────────────────────────────────────────────

#[test]
fn missing_required_attr_errors() {
    let src = r#"<task status="todo"><status>todo</status></task>"#;
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::MissingAttr { attr, .. } if attr == "id"
    )));
}

#[test]
fn missing_optional_attr_is_fine() {
    let src = r#"<task id="1"><status>todo</status></task>"#;
    let doc = parse(src).unwrap();
    // `status` attr is optional (no .required()).
    let report = validate(&doc, &task_schema());
    assert!(report.is_valid(), "got {:?}", report.errors());
}

// ─── InvalidAttr ────────────────────────────────────────────────────────────

#[test]
fn invalid_enum_value_errors() {
    let src = r#"<task id="1" status="bogus"><status>x</status></task>"#;
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::InvalidAttr { attr, value, .. }
            if attr == "status" && value == "bogus"
    )));
}

#[test]
fn regex_constraint_rejects_non_matching_value() {
    let schema = Schema::builder()
        .tag("task", |t| {
            t.attr("id", AttrKind::Regex(r"^\d+\.\d+$".into()).required())
        })
        .build();
    let doc = parse(r#"<task id="not-a-number"/>"#).unwrap();
    let errors = validate(&doc, &schema).errors().to_vec();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::InvalidAttr { reason, .. } if reason.contains("regex")
    )));
}

#[test]
fn regex_constraint_accepts_matching_value() {
    let schema = Schema::builder()
        .tag("task", |t| {
            t.attr("id", AttrKind::Regex(r"^\d+\.\d+$".into()).required())
        })
        .build();
    let doc = parse(r#"<task id="4.1"/>"#).unwrap();
    let report = validate(&doc, &schema);
    assert!(report.is_valid());
}

#[test]
fn string_kind_accepts_any_value() {
    let schema = Schema::builder()
        .tag("task", |t| t.attr("id", AttrKind::String.required()))
        .build();
    let doc = parse(r#"<task id="anything goes"/>"#).unwrap();
    assert!(validate(&doc, &schema).is_valid());
}

// ─── MissingChild ───────────────────────────────────────────────────────────

#[test]
fn missing_required_child_errors() {
    // No <status> child.
    let src = r#"<task id="1" status="todo">body</task>"#;
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::MissingChild { child, .. } if child == "status"
    )));
}

// ─── UnexpectedChild ────────────────────────────────────────────────────────

#[test]
fn unexpected_child_errors_only_when_exclusive() {
    let permissive_schema = Schema::builder()
        .tag("task", |t| t.child_required("status"))
        .build();
    let doc = parse("<task><status/><stowaway/></task>").unwrap();
    assert!(validate(&doc, &permissive_schema).is_valid());

    let strict_schema = Schema::builder()
        .tag("task", |t| t.child_required("status").exclusive_children())
        .build();
    let errors = validate(&doc, &strict_schema).errors().to_vec();
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::UnexpectedChild { child, .. } if child == "stowaway"
    )));
}

#[test]
fn optional_child_in_exclusive_list_is_allowed() {
    let schema = Schema::builder()
        .tag("task", |t| {
            t.child_required("status")
                .child_optional("note")
                .exclusive_children()
        })
        .build();
    let doc = parse("<task><status/><note/></task>").unwrap();
    assert!(validate(&doc, &schema).is_valid());
}

// ─── EmptyContent ───────────────────────────────────────────────────────────

#[test]
fn empty_content_errors_when_required() {
    let src = r#"<task id="1" status="todo"><status>x</status></task>"#;
    let doc = parse(src).unwrap();
    // `task` requires content. Element has only child tags + whitespace,
    // not text content — so it's empty.
    // Actually our schema accepts children OR text; the check fails only
    // when both children empty and content blank. Let's prove the negative.
    let report = validate(&doc, &task_schema());
    assert!(report.is_valid()); // has a <status> child, so not empty.

    let src2 = r#"<task id="1" status="todo">     </task>"#;
    let doc2 = parse(src2).unwrap();
    let errors = validate(&doc2, &task_schema()).errors().to_vec();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::EmptyContent { .. })));
    // Also MissingChild for the absent <status>.
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::MissingChild { .. })));
}

// ─── Deep validation ────────────────────────────────────────────────────────

#[test]
fn validates_at_every_depth() {
    let src = r#"<phase id="1"><task status="bogus"><status>todo</status></task></phase>"#;
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    // Nested task is missing `id` and has invalid status.
    assert!(errors.iter().any(|e| matches!(
        e,
        ValidationError::MissingAttr { attr, .. } if attr == "id"
    )));
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::InvalidAttr { .. })));
}

// ─── Builder + Display ──────────────────────────────────────────────────────

#[test]
fn validation_error_display() {
    let src = "<task><status>x</status></task>";
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    let messages: Vec<String> = errors.iter().map(ToString::to_string).collect();
    assert!(messages
        .iter()
        .any(|m| m.contains("missing required attribute")));
}

#[test]
fn attr_kind_optional_via_method_and_via_into() {
    // Both `.optional()` and bare `Into<AttrConstraint>` produce the
    // same shape.
    let a = AttrKind::String.optional();
    let b: marxml::schema::AttrConstraint = AttrKind::String.into();
    // Round-trip through a schema and validate the same input either way.
    let schema_a = Schema::builder().tag("task", |t| t.attr("foo", a)).build();
    let schema_b = Schema::builder().tag("task", |t| t.attr("foo", b)).build();
    let doc = parse("<task/>").unwrap();
    assert!(validate(&doc, &schema_a).is_valid());
    assert!(validate(&doc, &schema_b).is_valid());
}

// ─── Build-time regex validation ────────────────────────────────────────────

#[test]
#[should_panic(expected = "invalid regex")]
fn build_panics_on_invalid_regex_pattern() {
    let _ = Schema::builder()
        .tag("task", |t| {
            t.attr("id", AttrKind::Regex("[".into()).required())
        })
        .build();
}
