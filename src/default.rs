pub(crate) fn _default_to<'a, T: Default + Clone, U>(
    prefer: &Option<T>,
    base: Option<U>,
    f: fn(U) -> &'a T,
) -> T {
    prefer
        .clone()
        .or(base.map(f).cloned())
        .unwrap_or_default()
}

pub(crate) fn _default_to_with_default<T: Default + Clone, U>(
    prefer: Option<T>,
    base: Option<U>,
    f: fn(U) -> T,
    default: T,
) -> T {
    prefer
        .clone()
        .or(base.map(f))
        .unwrap_or(default)
}

pub(crate) fn _default_optional<'a, T: Clone, U>(
    prefer: &Option<T>,
    base: Option<U>,
    f: fn(U) -> &'a Option<T>,
) -> Option<T> {
    prefer.clone().or(base.map(f).cloned().flatten())
}

macro_rules! default_to {
    ($prefer:expr, $base:expr, $f:ident) => {
        crate::default::_default_to(&$prefer.$f, $base, |b| &b.$f)
    };
    ($prefer:expr, $base:expr, $f:ident, $def:expr) => {
        crate::default::_default_to_with_default($prefer.$f, $base, |b| b.$f, $def)
    };
}

macro_rules! default_optional {
    ($prefer:expr, $base:expr, $f:ident) => {
        crate::default::_default_optional(&$prefer.$f, $base, |b| &b.$f)
    };
}

pub(crate) use default_optional;
pub(crate) use default_to;
