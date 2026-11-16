/// A reference which may live for the output or only for a local computation.
#[derive(Clone, Copy, Debug)]
pub enum MultiLife<'out: 'local, 'local, T: ?Sized> {
    Out(&'out T),
    Local(&'local T),
}

impl<'out: 'local, 'local, T: ?Sized> std::ops::Deref for MultiLife<'out, 'local, T> {
    type Target = T;
    fn deref(&self) -> &T {
        match self {
            Self::Out(value) => value,
            Self::Local(value) => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unsized_references() {
        let local = String::from("local");
        let out: MultiLife<'static, '_, str> = MultiLife::Out("output");
        let local: MultiLife<'static, '_, str> = MultiLife::Local(&local);
        assert_eq!(&*out, "output");
        assert_eq!(&*local, "local");
    }
}
