// RFC 093 A3.2 negative fixture, the G07-vs-G07b proof pair: the lint
// violation exists only in the "ldap"-absent branch, mirroring
// crates/sui-id/src/runtime/startup.rs's real one (the reason G07b was
// added to RFC 093 at all). Under --all-features (G07), "ldap" is active
// and this branch is not even compiled, so G07 must pass. Under default
// features (G07b), "ldap" is inactive and the branch IS compiled and
// linted, so G07b must fail.
#[cfg(not(feature = "ldap"))]
pub fn is_enabled(flag: bool) -> bool {
    if flag == true { true } else { false }
}

#[cfg(feature = "ldap")]
pub fn is_enabled(flag: bool) -> bool {
    flag
}
