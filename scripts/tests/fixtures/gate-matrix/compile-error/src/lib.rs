// RFC 093 A3.2 negative fixture: a genuine compile error (type mismatch),
// not a lint or a test failure. Must make G01/G03/G05/G06 fail at the
// build step.
pub fn broken() -> u32 {
    "this is not a u32"
}
