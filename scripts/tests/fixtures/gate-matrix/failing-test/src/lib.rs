// RFC 093 A3.2 negative fixture: the crate builds cleanly; only a test
// assertion fails. Must make G02/G04/G05/G06 fail at the test step, and
// must NOT make G01/G03 (build-only) fail.
pub fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deliberately_wrong() {
        assert_eq!(add(2, 2), 5, "2 + 2 must not equal 5");
    }
}
