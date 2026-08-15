//! Integration tests for the public miniconf API.

use miniconf::{Config, ParseError, Value};

const SAMPLE: &str = "\
# demo configuration
title = miniconf demo

[server]
host = localhost
port = 8080
tls = off

[logging]
level = debug
";

#[test]
fn parses_sample_document() {
    let config = Config::parse(SAMPLE).unwrap();
    assert_eq!(config.get("", "title").unwrap().as_str(), "miniconf demo");
    assert_eq!(config.get("server", "port").unwrap().as_int(), Some(8080));
    assert_eq!(config.get("logging", "level").unwrap().as_str(), "debug");
    assert!(config.get("server", "missing").is_none());
    assert!(config.get("nope", "host").is_none());
}

#[test]
fn sections_are_sorted_and_include_root() {
    let config = Config::parse(SAMPLE).unwrap();
    let names: Vec<&str> = config.sections().collect();
    assert_eq!(names, vec!["", "logging", "server"]);
}

#[test]
fn entries_are_sorted_by_key() {
    let config = Config::parse(SAMPLE).unwrap();
    let keys: Vec<&str> = config.entries("server").map(|(k, _)| k).collect();
    assert_eq!(keys, vec!["host", "port", "tls"]);
}

#[test]
fn lookup_compares_equal_to_constructed_value() {
    let config = Config::parse(SAMPLE).unwrap();
    assert_eq!(config.get("server", "port"), Some(&Value::new("8080")));
}

#[test]
fn parse_errors_surface_with_line_numbers() {
    let err = Config::parse("[server]\noops\n").unwrap_err();
    assert_eq!(err, ParseError::MissingEquals { line: 2 });
}
