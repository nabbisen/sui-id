use super::*;

/// RFC 148 §1 (O2b): a comment can never hide a value that differs from
/// `Config::sample()` -- parsing the rendered local-trial profile back
/// must produce exactly that struct, field for field.
#[test]
fn local_trial_toml_round_trips_to_config_sample() {
    let text = local_trial_toml();
    let parsed: Config = toml::from_str(&text).expect("local_trial_toml must parse");
    let sample = Config::sample();

    assert_eq!(parsed.server.listen_addr, sample.server.listen_addr);
    assert_eq!(parsed.server.issuer, sample.server.issuer);
    assert_eq!(parsed.server.cookie_secure, sample.server.cookie_secure);
    assert_eq!(parsed.server.trusted_proxies, sample.server.trusted_proxies);
    assert_eq!(parsed.server.metrics_enabled, sample.server.metrics_enabled);
    assert_eq!(
        parsed.server.metrics_listen_addr,
        sample.server.metrics_listen_addr
    );
    assert_eq!(parsed.storage.db_path, sample.storage.db_path);
    assert_eq!(parsed.storage.key_file, sample.storage.key_file);
    assert_eq!(
        parsed.tokens.access_lifetime_secs,
        sample.tokens.access_lifetime_secs
    );
    assert_eq!(
        parsed.tokens.id_token_lifetime_secs,
        sample.tokens.id_token_lifetime_secs
    );
    assert_eq!(
        parsed.tokens.refresh_lifetime_secs,
        sample.tokens.refresh_lifetime_secs
    );
    assert_eq!(parsed.log.format, sample.log.format);
    assert_eq!(parsed.log.filter, sample.log.filter);
    assert_eq!(parsed.log.access_log, sample.log.access_log);
    assert_eq!(parsed.security.max_lockout, sample.security.max_lockout);
    assert_eq!(parsed.user_sources.len(), sample.user_sources.len());
    assert_eq!(
        parsed.federation_providers.len(),
        sample.federation_providers.len()
    );
}

/// The rendered text actually has comments -- the measured defect this
/// dispatch exists to fix (`config sample` had none at all).
#[test]
fn local_trial_toml_is_commented() {
    let text = local_trial_toml();
    let comment_lines = text
        .lines()
        .filter(|l| l.trim_start().starts_with('#'))
        .count();
    assert!(
        comment_lines >= 10,
        "expected the local-trial profile to carry real commentary, found \
         {comment_lines} comment line(s):\n{text}"
    );
}

#[test]
fn production_toml_is_the_example_file_and_parses() {
    let text = production_toml();
    assert!(text.starts_with("# sui-id configuration file."));
    // RFC 147 §4's own test already holds this file to `Config::sample()`'s
    // keys; this only confirms the embedded copy is the same file, not a
    // stale build-time snapshot.
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let on_disk =
        std::fs::read_to_string(std::path::Path::new(manifest_dir).join("sui-id.example.toml"))
            .expect("read sui-id.example.toml");
    assert_eq!(text, on_disk);
}
