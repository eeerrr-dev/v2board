use super::*;

#[test]
fn generated_database_names_are_quote_safe_and_bounded() {
    let name = GeneratedDatabaseName::new("contract").unwrap();
    assert!(name.as_str().starts_with("v2board_contract_"));
    assert!(name.as_str().len() <= 63);
    assert_eq!(name.quoted(), format!("\"{}\"", name.as_str()));
    assert!(validate_generated_database_name("bad\";drop database postgres").is_err());
    assert!(validate_generated_database_name("Uppercase").is_err());
}
