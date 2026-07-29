// RFC 093 A3.2 negative fixture: compiles, tests, and lints clean, but is
// not rustfmt-formatted (irregular spacing/indentation). Must make G08 fail
// and must NOT make G01-G07b fail.
pub fn add(a:u32,b:u32)->u32{
        a+b
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn it_adds() { assert_eq!(add(2,2),4); }
}
