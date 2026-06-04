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

/// Same as [`task_schema`] but without `content_required`, for tests that
/// focus on attribute or child rules and don't care about text body.
fn task_schema_no_content() -> Schema {
    Schema::builder()
        .tag("task", |t| {
            t.attr("id", AttrKind::String.required())
                .attr("status", AttrKind::Enum(vec!["todo".into(), "done".into()]))
                .child_required("status")
        })
        .build()
}

// ─── Happy path ─────────────────────────────────────────────────────────────

#[test]
fn fully_compliant_doc_validates() {
    // `task` requires direct text content as well as a `<status>` child, so
    // the body carries both.
    let src = r#"<task id="1" status="todo">buy milk<status>todo</status></task>"#;
    let doc = parse(src).unwrap();
    let report = validate(&doc, &task_schema());
    assert!(report.is_valid(), "got {:?}", report.errors());
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
    // No-content schema variant — this test only cares that the optional
    // `status` attribute can be absent.
    let src = r#"<task id="1"><status>todo</status></task>"#;
    let doc = parse(src).unwrap();
    let report = validate(&doc, &task_schema_no_content());
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
        ValidationError::InvalidAttr {
            kind: marxml::InvalidAttrKind::NoRegexMatch { .. },
            ..
        }
    )));
}

#[test]
fn regex_constraint_is_anchored_full_match() {
    // `todo|done` must reject `undone` and `done!` — the schema regex is
    // anchored automatically so partial matches don't slip through.
    let schema = Schema::builder()
        .tag("task", |t| {
            t.attr("status", AttrKind::Regex("todo|done".into()).required())
        })
        .build();
    let bad = parse(r#"<task status="undone"/>"#).unwrap();
    assert!(!validate(&bad, &schema).is_valid());
    let ok = parse(r#"<task status="done"/>"#).unwrap();
    assert!(validate(&ok, &schema).is_valid());
}

#[test]
fn duplicate_tag_in_builder_errors() {
    let result = Schema::builder()
        .tag("task", |t| t.attr("id", AttrKind::String.required()))
        .tag("task", |t| t.child_required("status"))
        .try_build();
    assert!(matches!(
        result,
        Err(marxml::SchemaError::DuplicateTag { .. })
    ));
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
    // Whitespace-only body fails content_required.
    let src = r#"<task id="1" status="todo">     </task>"#;
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::EmptyContent { .. })));
    // Also MissingChild for the absent <status>.
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::MissingChild { .. })));
}

#[test]
fn comment_only_body_fails_content_required() {
    // `<!-- ... -->` is trivia, not text. content_required must reject it.
    let src = r#"<task id="1" status="todo"><!--just a note--><status>todo</status></task>"#;
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::EmptyContent { .. })));
}

#[test]
fn structural_only_body_fails_content_required() {
    // `content_required` is about *text* content. Child-element markup
    // alone does not satisfy the rule — `<task>...<status/>...</task>` with
    // no direct text errors out.
    let src = r#"<task id="1" status="todo"><status>todo</status></task>"#;
    let doc = parse(src).unwrap();
    let errors = validate(&doc, &task_schema()).errors().to_vec();
    assert!(errors
        .iter()
        .any(|e| matches!(e, ValidationError::EmptyContent { .. })));
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
