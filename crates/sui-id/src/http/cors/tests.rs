use super::*;

#[test]
fn parse_origin_reads_scheme_host_port() {
    assert_eq!(
        parse_origin("https://app.example.com/cb"),
        Some(("https".into(), "app.example.com".into(), None))
    );
    assert_eq!(
        parse_origin("http://localhost:3000/cb?x=1"),
        Some(("http".into(), "localhost".into(), Some(3000)))
    );
}

#[test]
fn parse_origin_rejects_non_http_schemes() {
    assert_eq!(parse_origin("ftp://x"), None);
    assert_eq!(parse_origin("javascript:alert(1)"), None);
    assert_eq!(parse_origin("file:///etc/passwd"), None);
}

#[test]
fn parse_origin_lowercases_host_and_scheme() {
    // Origins are case-insensitive on scheme and host but the
    // browser-supplied `Origin` header tends to be lower-case.
    // Our equality check is case-sensitive on the parsed tuple,
    // so normalise both sides.
    let a = parse_origin("HTTPS://Example.COM/x");
    let b = parse_origin("https://example.com/y");
    assert_eq!(a, b);
}

#[test]
fn parse_origin_rejects_empty_authority() {
    assert_eq!(parse_origin("https:///path"), None);
}
