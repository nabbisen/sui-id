use super::*;

fn t(secs: i64) -> DateTime<Utc> {
    DateTime::from_timestamp(secs, 0).expect("valid epoch")
}

#[test]
fn first_request_is_allowed() {
    let l: Limiter<IpAddr> = Limiter::new(3, 60);
    let d = l.check("k", "127.0.0.1".parse().unwrap(), t(0));
    assert!(d.allowed);
    assert_eq!(d.remaining, 2);
}

#[test]
fn limit_blocks_within_window() {
    let l: Limiter<IpAddr> = Limiter::new(2, 60);
    let ip = "127.0.0.1".parse().unwrap();
    assert!(l.check("k", ip, t(0)).allowed);
    assert!(l.check("k", ip, t(1)).allowed);
    let d = l.check("k", ip, t(2));
    assert!(!d.allowed);
    assert!(d.retry_after_secs > 0);
}

#[test]
fn limit_resets_after_window() {
    let l: Limiter<IpAddr> = Limiter::new(1, 60);
    let ip = "127.0.0.1".parse().unwrap();
    assert!(l.check("k", ip, t(0)).allowed);
    assert!(!l.check("k", ip, t(30)).allowed);
    // After the window, fresh count.
    assert!(l.check("k", ip, t(61)).allowed);
}

#[test]
fn different_ips_are_independent() {
    let l: Limiter<IpAddr> = Limiter::new(1, 60);
    let a = "10.0.0.1".parse().unwrap();
    let b = "10.0.0.2".parse().unwrap();
    assert!(l.check("k", a, t(0)).allowed);
    assert!(l.check("k", b, t(0)).allowed);
    assert!(!l.check("k", a, t(1)).allowed);
}

#[test]
fn different_keys_are_independent() {
    let l: Limiter<IpAddr> = Limiter::new(1, 60);
    let ip = "10.0.0.1".parse().unwrap();
    assert!(l.check("login", ip, t(0)).allowed);
    assert!(l.check("token", ip, t(0)).allowed);
    assert!(!l.check("login", ip, t(1)).allowed);
}

/// RFC 123: the same mechanism, keyed on a `String` claimed client id
/// rather than an `IpAddr` — the generic bound (`Eq + Hash + Clone`) is
/// what makes this possible without a second, duplicated struct.
#[test]
fn a_string_keyed_limiter_behaves_the_same_as_an_ip_keyed_one() {
    let l: Limiter<String> = Limiter::new(1, 60);
    assert!(l.check("k", "client-a".to_owned(), t(0)).allowed);
    assert!(!l.check("k", "client-a".to_owned(), t(1)).allowed);
    assert!(l.check("k", "client-b".to_owned(), t(1)).allowed);
}
