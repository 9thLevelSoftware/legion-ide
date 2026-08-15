//! miniconf: a tiny INI-style configuration parser with no dependencies.
//!
//! ```
//! let config = miniconf::Config::parse("[server]\nport = 8080\n").unwrap();
//! assert_eq!(config.get("server", "port").unwrap().as_int(), Some(8080));
//! ```

mod error;
mod parser;

pub use error::ParseError;

use std::collections::BTreeMap;

/// A single configuration value, stored as raw text with typed accessors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Value {
    raw: String,
}

impl Value {
    /// Wrap raw value text.
    pub fn new(raw: impl Into<String>) -> Self {
        Value { raw: raw.into() }
    }

    /// The raw text of the value, as written in the source.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Parse the value as a signed integer.
    pub fn as_int(&self) -> Option<i64> {
        self.raw.parse().ok()
    }

    /// Parse the value as a boolean. Accepts `true`/`false`, `yes`/`no`,
    /// and `on`/`off`, case-insensitively.
    pub fn as_bool(&self) -> Option<bool> {
        match self.raw.to_ascii_lowercase().as_str() {
            "true" | "yes" | "on" => Some(true),
            "false" | "no" | "off" => Some(false),
            _ => None,
        }
    }
}

/// A parsed configuration: sections mapping keys to values.
///
/// The root section (keys assigned before any `[section]` header) is stored
/// under the empty string.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Config {
    sections: BTreeMap<String, BTreeMap<String, Value>>,
}

impl Config {
    /// Parse configuration text. See the crate docs for the grammar.
    pub fn parse(text: &str) -> Result<Config, ParseError> {
        parser::parse(text)
    }

    pub(crate) fn insert(&mut self, section: &str, key: &str, value: Value) {
        self.sections
            .entry(section.to_string())
            .or_default()
            .insert(key.to_string(), value);
    }

    /// Look up a value by section and key. The root section is `""`.
    pub fn get(&self, section: &str, key: &str) -> Option<&Value> {
        self.sections.get(section)?.get(key)
    }

    /// Names of all sections that contain at least one key, sorted.
    /// Includes the root section `""` when it is non-empty.
    pub fn sections(&self) -> impl Iterator<Item = &str> {
        self.sections.keys().map(String::as_str)
    }

    /// Iterate over `(key, value)` pairs of one section, sorted by key.
    pub fn entries(&self, section: &str) -> impl Iterator<Item = (&str, &Value)> {
        self.sections
            .get(section)
            .into_iter()
            .flat_map(|entries| entries.iter().map(|(k, v)| (k.as_str(), v)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value_as_str_returns_raw_text() {
        assert_eq!(Value::new("hello world").as_str(), "hello world");
    }

    #[test]
    fn value_as_int_parses_integers() {
        assert_eq!(Value::new("42").as_int(), Some(42));
        assert_eq!(Value::new("-7").as_int(), Some(-7));
        assert_eq!(Value::new("4.2").as_int(), None);
        assert_eq!(Value::new("forty-two").as_int(), None);
    }

    #[test]
    fn config_default_is_empty() {
        let config = Config::default();
        assert_eq!(config.sections().count(), 0);
        assert!(config.get("", "anything").is_none());
    }
}
