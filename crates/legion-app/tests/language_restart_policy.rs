use legion_app::language::RestartPolicy;

#[test]
fn restart_backoff_grows_until_cap() {
    // Uses a session-free policy check via the public helper on the policy.
    let policy = RestartPolicy { max_restarts: 2, backoff_base_ms: 100 };
    assert_eq!(policy.backoff_for_attempt(0).as_millis(), 100);
    assert_eq!(policy.backoff_for_attempt(1).as_millis(), 200);
    assert!(policy.is_exhausted(2));
}
