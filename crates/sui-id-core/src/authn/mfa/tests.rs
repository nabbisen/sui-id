use super::*;

#[tokio::test]
async fn recovery_code_format() {
    let c = generate_recovery_code().expect("recovery code");
    assert_eq!(c.len(), 17);
    assert_eq!(c.as_bytes()[5], b'-');
    assert_eq!(c.as_bytes()[11], b'-');
}
