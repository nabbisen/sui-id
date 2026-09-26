use super::*;
use std::path::PathBuf;

fn p() -> PathBuf {
    PathBuf::from("/var/lib/sui-id/sui-id.sqlite")
}

#[test]
fn a_too_new_line_names_the_path_both_versions_the_route_and_what_not_to_do() {
    let line = too_new_line(&p(), 44, 43, Some("0.79.0"));
    assert!(line.starts_with("sui-id: refusing to run:"), "{line}");
    assert!(line.contains("/var/lib/sui-id/sui-id.sqlite"), "the path");
    assert!(line.contains("schema version 44"), "the found version");
    assert!(line.contains("understands up to 43"), "the ceiling");
    assert!(line.contains(env!("CARGO_PKG_VERSION")), "this binary");
    assert!(line.contains("last migrated by sui-id 0.79.0"), "the stamp");
    assert!(
        line.contains("Run sui-id 0.79.0 or newer"),
        "the route forward"
    );
    assert!(line.contains("sui-id restore"), "the route back");
    assert!(line.contains("--force"), "the route back is runnable");
    assert!(line.contains("Nothing was changed"), "{line}");
    assert!(
        line.contains("Do not edit or delete the database"),
        "{line}"
    );
    assert!(line.contains("do not run migrations by hand"), "{line}");
    // The words the deployment guide's grep looks for.
    assert!(line.contains("schema") && line.contains("migrat"), "{line}");
}

#[test]
fn without_a_stamp_the_line_says_a_newer_sui_id() {
    let line = too_new_line(&p(), 44, 43, None);
    assert!(line.contains("Run a newer sui-id,"), "{line}");
    assert!(!line.contains("last migrated by"), "{line}");
}

#[test]
fn an_invalid_line_names_the_path_the_detail_and_the_table_count() {
    let line = invalid_line(
        &p(),
        24,
        "the recorded value \"garbage\" is not a non-negative integer",
    );
    assert!(line.starts_with("sui-id: refusing to run:"), "{line}");
    assert!(line.contains("/var/lib/sui-id/sui-id.sqlite"), "{line}");
    assert!(line.contains("\"garbage\""), "{line}");
    assert!(line.contains("24 application table(s)"), "{line}");
    assert!(line.contains("no migrations were run"), "{line}");
    assert!(line.contains("Nothing was changed"), "{line}");
    assert!(line.contains("sui-id restore"), "{line}");
    assert!(line.contains("schema") && line.contains("migrat"), "{line}");
}

#[test]
fn a_line_is_one_line_whatever_a_path_or_a_stored_value_contained() {
    let path = PathBuf::from("/tmp/a\nb\u{1b}[31m/db.sqlite");
    for line in [
        too_new_line(&path, 44, 43, Some("0.79.0\nX")),
        invalid_line(&path, 1, "a\r\nb"),
    ] {
        assert!(
            !line.contains('\n') && !line.contains('\r') && !line.contains('\u{1b}'),
            "{line:?}"
        );
    }
}

#[test]
fn only_the_two_schema_refusals_are_a_refusal() {
    let too_new = StoreError::SchemaTooNew {
        found: 44,
        supported: 43,
    };
    let invalid = StoreError::SchemaVersionInvalid {
        tables: 3,
        detail: "x".into(),
    };
    assert!(refusal(&p(), &too_new).is_some());
    assert!(refusal(&p(), &invalid).is_some());
    for other in [
        StoreError::NotFound,
        StoreError::Conflict,
        StoreError::Integrity("x".into()),
        StoreError::MigrationFailed {
            version: 2,
            source: rusqlite::Error::InvalidQuery,
        },
    ] {
        assert!(refusal(&p(), &other).is_none(), "{other:?}");
    }
}

#[test]
fn both_refusal_variants_share_one_exit_code() {
    // D5: a code covering one variant covers nothing.
    assert_eq!(EXIT_DATABASE_REFUSED, 65);
}

#[test]
fn a_refusal_survives_anyhow_context_and_downcasts() {
    let refused: anyhow::Error = refusal(
        &p(),
        &StoreError::SchemaTooNew {
            found: 44,
            supported: 43,
        },
    )
    .unwrap()
    .into();
    let wrapped = refused.context("some caller's wrapper");
    assert!(wrapped.downcast_ref::<DatabaseRefused>().is_some());
}
