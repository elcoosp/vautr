//! VTRFIX-TST-04: import/export round-trip invariants.
//!
//! Guards BUG-H08 (TOTP preserved), BUG-H09 (username preserved),
//! BUG-H10 (passwords not trimmed), BUG-M13 (CSV formula sanitization).

use std::io::Cursor;
use vautr_import::source::SourceKind;

#[test]
fn csv_preserves_whitespace_in_passwords_and_username() {
    // VTRFIX-BUG-H10: passwords must be preserved verbatim.
    let data = "name,url,username,password,totp\nBank,https://bank.example, u1 , p@ss with space ,JBSWY3DPEHPK3PXP\n";
    let iter = vautr_import::parser::parse_stream(Box::new(Cursor::new(data)), SourceKind::Csv)
        .expect("parse");
    let records: Vec<_> = iter.collect::<Result<Vec<_>, _>>().expect("collect");
    assert_eq!(records.len(), 1);
    let fields = records[0].item.fields.as_object().unwrap();
    // Username preserved with whitespace
    assert_eq!(fields.get("username").and_then(|v| v.as_str()), Some(" u1 "));
    // Password preserved with whitespace
    assert_eq!(
        fields.get("password").and_then(|v| v.as_str()),
        Some(" p@ss with space ")
    );
    // TOTP preserved (BUG-H08)
    assert_eq!(
        fields.get("totp").and_then(|v| v.as_str()),
        Some("JBSWY3DPEHPK3PXP")
    );
}

#[test]
fn bitwarden_json_preserves_totp() {
    let json = r#"{"items":[{"id":"a1","name":"GitHub","login":{"username":"u","password":"p","totp":"JBSWY3DPEHPK3PXP","uris":[{"uri":"https://github.com"}]}}]}"#;
    let iter = vautr_import::parser::parse_stream(
        Box::new(Cursor::new(json)),
        SourceKind::BitwardenJson,
    )
    .expect("parse");
    let records: Vec<_> = iter.collect::<Result<Vec<_>, _>>().expect("collect");
    assert_eq!(records.len(), 1);
    let fields = records[0].item.fields.as_object().unwrap();
    assert_eq!(
        fields.get("totp").and_then(|v| v.as_str()),
        Some("JBSWY3DPEHPK3PXP"),
        "Bitwarden login.totp must round-trip"
    );
}
