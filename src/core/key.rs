use std::fmt;
use std::sync::Arc;

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum KeyValue {
    Numeric(u64),
    Text(Arc<str>),
}

impl fmt::Debug for KeyValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Numeric(value) => value.fmt(formatter),
            Self::Text(value) => value.fmt(formatter),
        }
    }
}

macro_rules! stable_key {
    ($name:ident) => {
        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(KeyValue);

        impl $name {
            pub const fn numeric(value: u64) -> Self {
                Self(KeyValue::Numeric(value))
            }

            pub fn text(value: impl Into<Arc<str>>) -> Self {
                Self(KeyValue::Text(value.into()))
            }

            pub fn as_numeric(&self) -> Option<u64> {
                match self.0 {
                    KeyValue::Numeric(value) => Some(value),
                    KeyValue::Text(_) => None,
                }
            }

            pub fn as_str(&self) -> Option<&str> {
                match &self.0 {
                    KeyValue::Numeric(_) => None,
                    KeyValue::Text(value) => Some(value),
                }
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self::numeric(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::text(value)
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::text(value)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_tuple(stringify!($name))
                    .field(&self.0)
                    .finish()
            }
        }
    };
}

stable_key!(NodeKey);
stable_key!(WidgetKey);
stable_key!(ItemKey);

/// Stable identity of an animatable or invalidatable property.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyId(u64);

impl PropertyId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for PropertyId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl fmt::Debug for PropertyId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("PropertyId").field(&self.0).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn key_identity_includes_kind_and_value() {
        let numeric = WidgetKey::from(7);
        let text = WidgetKey::from("7");
        let mut keys = HashSet::new();
        keys.insert(numeric.clone());
        keys.insert(text.clone());

        assert_eq!(keys.len(), 2);
        assert_eq!(numeric.as_numeric(), Some(7));
        assert_eq!(text.as_str(), Some("7"));
        assert_eq!(format!("{text:?}"), "WidgetKey(\"7\")");
    }
}
