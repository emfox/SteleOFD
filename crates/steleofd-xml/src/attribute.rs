//! Attribute storage and typed access.
//! OFD attributes are all strings on the wire but typed in meaning, so accessors
//! convert on demand and name the element when conversion fails.

use steleofd_error::Result;

use crate::parse_optional;

/// Decoded attributes of one element, keyed by local name.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Attributes {
    entries: Vec<(String, String)>,
}

impl Attributes {
    /// An empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert or replace an attribute.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let key = key.into();
        let value = value.into();
        match self.entries.iter_mut().find(|(k, _)| *k == key) {
            // A duplicate attribute is malformed XML, but keeping the last keeps
            // behaviour predictable instead of failing the whole document.
            Some(slot) => slot.1 = value,
            None => self.entries.push((key, value)),
        }
    }

    /// Raw string value, if present.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// A required string attribute.
    pub fn require(&self, element: &str, key: &str) -> Result<&str> {
        self.get(key).ok_or_else(|| {
            steleofd_error::OfdError::Xml(steleofd_error::XmlError::MissingAttribute {
                element: element.to_string(),
                attr: key.to_string(),
            })
        })
    }

    /// A required typed attribute.
    pub fn require_as<T>(&self, element: &str, key: &str) -> Result<T>
    where
        T: std::str::FromStr,
    {
        crate::parse_required::<T>(element, key, self.get(key))
    }

    /// An optional typed attribute.
    pub fn optional_as<T>(&self, element: &str, key: &str) -> Result<Option<T>>
    where
        T: std::str::FromStr,
    {
        parse_optional::<T>(element, key, self.get(key))
    }

    /// A typed attribute falling back to a default when absent.
    pub fn or_default<T>(&self, element: &str, key: &str, default: T) -> Result<T>
    where
        T: std::str::FromStr,
    {
        Ok(self.optional_as::<T>(element, key)?.unwrap_or(default))
    }

    /// Number of attributes.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether there are no attributes.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over the raw pairs in insertion order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.entries.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }
}

/// One attribute value, decoded but not yet converted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttrValue {
    /// Attribute local name.
    pub name: String,
    /// Decoded value.
    pub value: String,
}

impl AttrValue {
    /// Build a value.
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }

    /// Convert to `T`, naming this attribute on failure.
    pub fn parse<T>(&self, element: &str) -> Result<T>
    where
        T: std::str::FromStr,
    {
        crate::parse_required::<T>(element, &self.name, Some(&self.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Attributes {
        let mut a = Attributes::new();
        a.insert("ID", "7");
        a.insert("Boundary", "0 0 210 297");
        a.insert("Visible", "true");
        a
    }

    #[test]
    fn values_are_read_by_local_name() {
        let a = sample();
        assert_eq!(a.get("ID"), Some("7"));
        assert_eq!(a.get("Nope"), None);
    }

    #[test]
    fn required_strings_report_missing_keys() {
        let a = sample();
        assert_eq!(a.require("Page", "ID").expect("found"), "7");
        let err = a.require("Page", "BaseLoc").expect_err("must fail");
        match err {
            steleofd_error::OfdError::Xml(steleofd_error::XmlError::MissingAttribute {
                element,
                attr,
            }) => {
                assert_eq!(element, "Page");
                assert_eq!(attr, "BaseLoc");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn typed_required_conversion_names_the_element() {
        let a = sample();
        assert_eq!(a.require_as::<u32>("Page", "ID").expect("parse"), 7);

        let mut bad = Attributes::new();
        bad.insert("ID", "seven");
        let err = bad.require_as::<u32>("Page", "ID").expect_err("must fail");
        match err {
            steleofd_error::OfdError::Xml(steleofd_error::XmlError::InvalidAttribute {
                element,
                value,
                ..
            }) => {
                assert_eq!(element, "Page");
                assert_eq!(value, "seven");
            }
            other => panic!("unexpected error: {other}"),
        }
    }

    #[test]
    fn optional_conversion_passes_absence_through() {
        let a = sample();
        assert_eq!(
            a.optional_as::<bool>("Page", "Visible").expect("parse"),
            Some(true)
        );
        assert_eq!(
            a.optional_as::<bool>("Page", "Missing").expect("parse"),
            None
        );
    }

    #[test]
    fn defaults_apply_only_when_absent() {
        let a = sample();
        assert!(a.or_default("Page", "Visible", false).expect("parse"));
        assert!(!a.or_default("Page", "Missing", false).expect("parse"));
    }

    #[test]
    fn inserting_the_same_key_twice_keeps_the_last() {
        let mut a = Attributes::new();
        a.insert("ID", "1");
        a.insert("ID", "2");
        assert_eq!(a.get("ID"), Some("2"));
        assert_eq!(a.len(), 1);
    }

    #[test]
    fn iteration_preserves_insertion_order() {
        let a = sample();
        let keys: Vec<&str> = a.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, vec!["ID", "Boundary", "Visible"]);
    }

    #[test]
    fn emptiness_is_reported() {
        let a = Attributes::new();
        assert!(a.is_empty());
        assert_eq!(a.len(), 0);

        let b = sample();
        assert!(!b.is_empty());
        assert_eq!(b.len(), 3);
    }

    #[test]
    fn attr_value_converts_with_its_own_name_in_the_error() {
        let v = AttrValue::new("Boundary", "0 0 210 297");
        assert_eq!(v.name, "Boundary");

        let bad = AttrValue::new("PageCount", "many");
        let err = bad.parse::<u32>("Document").expect_err("must fail");
        match err {
            steleofd_error::OfdError::Xml(steleofd_error::XmlError::InvalidAttribute {
                element,
                attr,
                ..
            }) => {
                assert_eq!(element, "Document");
                assert_eq!(attr, "PageCount");
            }
            other => panic!("unexpected error: {other}"),
        }
    }
}
