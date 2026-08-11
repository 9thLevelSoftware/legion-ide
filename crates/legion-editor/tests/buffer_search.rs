use legion_editor::BufferSearchState;

#[test]
fn find_literal_matches() {
    let mut state = BufferSearchState::default();
    state.query = "hello".into();
    let count = state.find_matches("hello world hello");
    assert_eq!(count, 2);
    assert_eq!(state.matches[0], (0, 0, 0, 5));
    assert_eq!(state.matches[1], (0, 12, 0, 17));
}

#[test]
fn find_case_insensitive_default() {
    let mut state = BufferSearchState::default();
    state.query = "Hello".into();
    let count = state.find_matches("hello HELLO Hello");
    assert_eq!(count, 3);
}

#[test]
fn find_case_sensitive() {
    let mut state = BufferSearchState::default();
    state.query = "Hello".into();
    state.case_sensitive = true;
    let count = state.find_matches("hello HELLO Hello");
    assert_eq!(count, 1);
    assert_eq!(state.matches[0], (0, 12, 0, 17));
}

#[test]
fn find_whole_word() {
    let mut state = BufferSearchState::default();
    state.query = "he".into();
    state.whole_word = true;
    let count = state.find_matches("he hello she he");
    assert_eq!(count, 2);
}

#[test]
fn find_regex_mode() {
    let mut state = BufferSearchState::default();
    state.query = r"\d+".into();
    state.use_regex = true;
    let count = state.find_matches("abc 123 def 456");
    assert_eq!(count, 2);
}

#[test]
fn find_invalid_regex_returns_zero() {
    let mut state = BufferSearchState::default();
    state.query = r"[invalid".into();
    state.use_regex = true;
    let count = state.find_matches("some text");
    assert_eq!(count, 0);
}

#[test]
fn find_empty_query_returns_zero() {
    let mut state = BufferSearchState::default();
    let count = state.find_matches("some text");
    assert_eq!(count, 0);
}

#[test]
fn find_multiline() {
    let mut state = BufferSearchState::default();
    state.query = "fn".into();
    let count = state.find_matches("fn main() {\n    fn helper() {\n    }\n}");
    assert_eq!(count, 2);
    assert_eq!(state.matches[0], (0, 0, 0, 2));
    assert_eq!(state.matches[1], (1, 4, 1, 6));
}

#[test]
fn next_prev_match_wraps() {
    let mut state = BufferSearchState::default();
    state.query = "x".into();
    state.find_matches("x x x");
    assert_eq!(state.current_match_index, 0);
    state.next_match();
    assert_eq!(state.current_match_index, 1);
    state.next_match();
    assert_eq!(state.current_match_index, 2);
    state.next_match();
    assert_eq!(state.current_match_index, 0);
    state.prev_match();
    assert_eq!(state.current_match_index, 2);
}

#[test]
fn no_matches_navigation_is_safe() {
    let mut state = BufferSearchState::default();
    state.query = "nonexistent".into();
    state.find_matches("some text");
    assert_eq!(state.matches.len(), 0);
    state.next_match();
    state.prev_match();
    assert!(state.current_match().is_none());
}
