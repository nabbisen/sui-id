// RFC 093 A3.2 negative fixture: compiles and tests clean, but trips an
// ordinary warn-by-default clippy lint (clippy::bool_comparison). Must
// make G07/G07b fail, and must NOT make G01-G06/G08 fail.
pub fn is_enabled(flag: bool) -> bool {
    if flag == true { true } else { false }
}
