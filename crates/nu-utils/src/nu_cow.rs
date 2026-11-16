use serde::{Deserialize, Deserializer, Serialize};
use std::fmt;

/// A value with independently chosen borrowed and owned representations.
#[derive(Clone, Serialize)]
#[serde(untagged)]
pub enum NuCow<B, O> {
    Borrowed(B),
    Owned(O),
}

impl<'de, B, O: Deserialize<'de>> Deserialize<'de> for NuCow<B, O> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        O::deserialize(deserializer).map(Self::Owned)
    }
}

impl<B: PartialEq + PartialEq<O>, O: PartialEq + PartialEq<B>> PartialEq for NuCow<B, O> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Borrowed(a), Self::Borrowed(b)) => a == b,
            (Self::Owned(a), Self::Owned(b)) => a == b,
            (Self::Borrowed(a), Self::Owned(b)) => a == b,
            (Self::Owned(a), Self::Borrowed(b)) => a == b,
        }
    }
}
impl<B: Eq + PartialEq<O>, O: Eq + PartialEq<B>> Eq for NuCow<B, O> {}

impl<B: fmt::Debug, O: fmt::Debug> fmt::Debug for NuCow<B, O> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Borrowed(v) if f.alternate() => f.debug_tuple("Borrowed").field(v).finish(),
            Self::Owned(v) if f.alternate() => f.debug_tuple("Owned").field(v).finish(),
            Self::Borrowed(v) => v.fmt(f),
            Self::Owned(v) => v.fmt(f),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn value_semantics() {
        let borrowed: NuCow<&str, String> = NuCow::Borrowed("hello");
        let owned = NuCow::Owned(String::from("hello"));
        assert_eq!(borrowed, owned);
        assert_eq!(owned, borrowed);
        assert_eq!(borrowed.clone(), owned);
        assert_eq!(format!("{borrowed:?}"), "\"hello\"");
        assert!(format!("{borrowed:#?}").starts_with("Borrowed("));
        assert!(format!("{owned:#?}").starts_with("Owned("));
        let json = serde_json::to_string(&borrowed).unwrap();
        assert_eq!(json, "\"hello\"");
        let decoded: NuCow<&str, String> = serde_json::from_str(&json).unwrap();
        assert!(matches!(decoded, NuCow::Owned(_)));
        assert_eq!(decoded, borrowed);
    }
}
