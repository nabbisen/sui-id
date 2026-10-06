use super::*;
use std::collections::HashMap;

fn test_source(slug: &str) -> InMemoryUserSource {
    let mut users = HashMap::new();
    users.insert(
        "alice".to_owned(),
        (
            "s3cr3t".to_owned(),
            "uid=alice,dc=test".to_owned(),
            Some("alice@example.com".to_owned()),
            Some("Alice".to_owned()),
        ),
    );
    InMemoryUserSource {
        slug: slug.to_owned(),
        users,
    }
}

#[tokio::test]
async fn known_user_correct_password_matches() {
    let src = test_source("test");
    let result = src.authenticate("alice", "s3cr3t").await.unwrap();
    let record = result.expect("must match");
    assert_eq!(record.stable_id, "uid=alice,dc=test");
    assert_eq!(record.display_username, "alice");
    assert_eq!(record.email.as_deref(), Some("alice@example.com"));
    assert_eq!(record.source_slug, "test");
}

#[tokio::test]
async fn unknown_user_returns_none() {
    let src = test_source("test");
    let result = src.authenticate("bob", "anything").await.unwrap();
    assert!(result.is_none(), "unknown user must return None");
}

#[tokio::test]
async fn wrong_password_returns_none() {
    let src = test_source("test");
    let result = src.authenticate("alice", "wrong").await.unwrap();
    assert!(
        result.is_none(),
        "wrong password must return None (indistinguishable from unknown user)"
    );
}

#[tokio::test]
async fn cascade_first_match_wins() {
    // Two sources; alice is in source-1, not source-2.
    let src1 = Arc::new(test_source("s1")) as Arc<dyn UserSource>;
    let empty = Arc::new(InMemoryUserSource {
        slug: "s2".to_owned(),
        users: HashMap::new(),
    }) as Arc<dyn UserSource>;
    let sources = vec![src1, empty];

    match cascade_sources(&sources, "alice", "s3cr3t").await {
        CascadeOutcome::Matched(r) => assert_eq!(r.source_slug, "s1"),
        CascadeOutcome::NotFound => panic!("expected match"),
    }
}

#[tokio::test]
async fn cascade_not_found_when_no_source_matches() {
    let src = Arc::new(test_source("s1")) as Arc<dyn UserSource>;
    let sources = vec![src];
    let outcome = cascade_sources(&sources, "unknown", "pw").await;
    assert!(matches!(outcome, CascadeOutcome::NotFound));
}

#[tokio::test]
async fn cascade_continues_past_transport_error() {
    // A source that always fails with a transport error should be skipped.
    struct BrokenSource;
    #[async_trait::async_trait]
    impl UserSource for BrokenSource {
        async fn authenticate(
            &self,
            _u: &str,
            _p: &str,
        ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
            Err(UserSourceError::Transport("connection refused".into()))
        }
        async fn authenticate_stable_id(
            &self,
            _s: &str,
            _p: &str,
        ) -> Result<Option<ExternalUserRecord>, UserSourceError> {
            Err(UserSourceError::Transport("connection refused".into()))
        }
        fn slug(&self) -> &str {
            "broken"
        }
    }

    let broken = Arc::new(BrokenSource) as Arc<dyn UserSource>;
    let working = Arc::new(test_source("working")) as Arc<dyn UserSource>;
    let sources = vec![broken, working];

    // alice is in the working source; the broken one should not block her.
    match cascade_sources(&sources, "alice", "s3cr3t").await {
        CascadeOutcome::Matched(r) => assert_eq!(r.source_slug, "working"),
        CascadeOutcome::NotFound => panic!("expected cascade to continue past broken source"),
    }
}
