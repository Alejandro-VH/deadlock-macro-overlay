use deadlock_overlay::screen_reader::parse_match_time;

#[test]
fn test_parse_match_time_standard() {
    assert_eq!(parse_match_time("05:30"), Some(330));
    assert_eq!(parse_match_time("00:00"), Some(0));
    assert_eq!(parse_match_time("12:45"), Some(765));
    assert_eq!(parse_match_time("1:05"), Some(65));
}

#[test]
fn test_parse_match_time_ocr_glitches() {
    // Semicolon instead of colon
    assert_eq!(parse_match_time("05;30"), Some(330));
    // Spaces around colon
    assert_eq!(parse_match_time("05 : 30"), Some(330));
    assert_eq!(parse_match_time("12: 45"), Some(765));
    // Uppercase 'O' instead of '0'
    assert_eq!(parse_match_time("O5:3O"), Some(330));
    // Surrounding text or symbols
    assert_eq!(parse_match_time("MATCH 08:15 SCORE"), Some(495));
}

#[test]
fn test_parse_match_time_invalid() {
    assert_eq!(parse_match_time("DEADLOCK"), None);
    assert_eq!(parse_match_time(""), None);
    assert_eq!(parse_match_time("99:99"), None); // 99 seconds is invalid
}
