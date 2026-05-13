use super::names::{add_normalization_query_variants, normalize_existing_filename};
use std::borrow::Cow;

#[test]
fn normalization_query_variants_borrow_ascii_candidates() {
    let names = vec!["report.txt".to_string(), "report (1).txt".to_string()];
    let variants = add_normalization_query_variants(&names);

    assert!(matches!(variants, Cow::Borrowed(_)));
    assert_eq!(variants.as_ref(), names.as_slice());
}

#[test]
fn normalization_query_variants_add_unicode_forms_only_when_needed() {
    let names = vec![
        "caf\u{00e9}.txt".to_string(),
        "cafe\u{0301}.txt".to_string(),
    ];
    let variants = add_normalization_query_variants(&names);

    assert!(matches!(variants, Cow::Owned(_)));
    assert_eq!(variants.as_ref().len(), 2);
    assert!(variants.as_ref().contains(&"caf\u{00e9}.txt".to_string()));
    assert!(variants.as_ref().contains(&"cafe\u{0301}.txt".to_string()));
}

#[test]
fn normalize_existing_filename_reuses_ascii_and_nfc_names() {
    assert_eq!(
        normalize_existing_filename("report.txt".to_string()),
        "report.txt"
    );
    assert_eq!(
        normalize_existing_filename("caf\u{00e9}.txt".to_string()),
        "caf\u{00e9}.txt"
    );
    assert_eq!(
        normalize_existing_filename("cafe\u{0301}.txt".to_string()),
        "caf\u{00e9}.txt"
    );
}
