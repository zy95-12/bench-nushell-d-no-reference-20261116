#[allow(unused_imports)]
use crate::uformat;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Borrow, fmt, ops::Deref};

/// A compact string with inline and static storage, and uniquely owned heap storage.
/// Cloning an owned heap string copies its bytes without atomic reference counting.
#[derive(Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct UniqueString(byteyarn::Yarn);

impl UniqueString {
    pub fn from_string(value: String) -> Self {
        Self(byteyarn::Yarn::from_string(value))
    }
    pub const fn from_static_str(value: &'static str) -> Self {
        Self(byteyarn::Yarn::from_static(value))
    }
    pub fn from_fmt(args: fmt::Arguments<'_>) -> Self {
        Self(byteyarn::Yarn::from_fmt(args))
    }
}
impl Deref for UniqueString {
    type Target = str;
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}
impl AsRef<str> for UniqueString {
    fn as_ref(&self) -> &str {
        self
    }
}
impl Borrow<str> for UniqueString {
    fn borrow(&self) -> &str {
        self
    }
}
impl fmt::Debug for UniqueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
impl fmt::Display for UniqueString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}
impl Serialize for UniqueString {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self)
    }
}
impl<'de> Deserialize<'de> for UniqueString {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self::from_string)
    }
}

#[macro_export]
macro_rules! uformat {
    ($($arg:tt)*) => { $crate::strings::UniqueString::from_fmt(format_args!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_and_serde() {
        if cfg!(target_pointer_width = "64") {
            assert_eq!(size_of::<UniqueString>(), 16);
            assert_eq!(size_of::<Option<UniqueString>>(), 16);
        }
        let short = UniqueString::from_string(String::from("tiny"));
        let base = &short as *const _ as usize;
        assert!((base..base + size_of::<UniqueString>()).contains(&(short.as_ptr() as usize)));
        static LONG: &str = "a static string long enough to exceed inline storage capacity";
        let literal = UniqueString::from_static_str(LONG);
        assert_eq!(literal.as_ptr(), LONG.as_ptr());
        let value = UniqueString::from_string(LONG.to_owned());
        let cloned = value.clone();
        assert_eq!(value, cloned);
        assert_ne!(value.as_ptr(), cloned.as_ptr());
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<UniqueString>(&encoded).unwrap(),
            value
        );
        assert_eq!(&*uformat!("hello {}", 42), "hello 42");
        assert_eq!(&*UniqueString::default(), "");
    }
}
