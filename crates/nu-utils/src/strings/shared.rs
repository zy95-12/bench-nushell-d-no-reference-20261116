#[allow(unused_imports)]
use crate::sformat;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::{borrow::Borrow, fmt, ops::Deref};

/// A compact string with inline and static storage, and reference-counted heap storage.
/// Cloning a heap string shares its allocation.
#[derive(Clone, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[repr(transparent)]
pub struct SharedString(lean_string::LeanString);

impl SharedString {
    pub fn from_string(value: String) -> Self {
        Self(lean_string::LeanString::from(value))
    }
    pub const fn from_static_str(value: &'static str) -> Self {
        Self(lean_string::LeanString::from_static_str(value))
    }
    pub fn from_fmt(args: fmt::Arguments<'_>) -> Self {
        use fmt::Write;
        let mut value = lean_string::LeanString::new();
        value.write_fmt(args).expect("formatting failed");
        Self(value)
    }
}
impl Deref for SharedString {
    type Target = str;
    fn deref(&self) -> &str {
        self.0.as_str()
    }
}
impl AsRef<str> for SharedString {
    fn as_ref(&self) -> &str {
        self
    }
}
impl Borrow<str> for SharedString {
    fn borrow(&self) -> &str {
        self
    }
}
impl fmt::Debug for SharedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
impl fmt::Display for SharedString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&**self, f)
    }
}
impl Serialize for SharedString {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self)
    }
}
impl<'de> Deserialize<'de> for SharedString {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d).map(Self::from_string)
    }
}

#[macro_export]
macro_rules! sformat {
    ($($arg:tt)*) => { $crate::strings::SharedString::from_fmt(format_args!($($arg)*)) };
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn storage_and_serde() {
        if cfg!(target_pointer_width = "64") {
            assert_eq!(size_of::<SharedString>(), 16);
            assert_eq!(size_of::<Option<SharedString>>(), 16);
        }
        let short = SharedString::from_string(String::from("tiny"));
        let base = &short as *const _ as usize;
        assert!((base..base + size_of::<SharedString>()).contains(&(short.as_ptr() as usize)));
        static LONG: &str = "a static string long enough to exceed inline storage capacity";
        let literal = SharedString::from_static_str(LONG);
        assert_eq!(literal.as_ptr(), LONG.as_ptr());
        let value = SharedString::from_string(LONG.to_owned());
        let cloned = value.clone();
        assert_eq!(value, cloned);
        assert_eq!(value.as_ptr(), cloned.as_ptr());
        let encoded = serde_json::to_string(&value).unwrap();
        assert_eq!(
            serde_json::from_str::<SharedString>(&encoded).unwrap(),
            value
        );
        assert_eq!(&*sformat!("hello {}", 42), "hello 42");
        assert_eq!(&*SharedString::default(), "");
    }
}
