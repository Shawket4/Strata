//! `string_enum!`: one declaration yields a fieldless enum whose wire, frontmatter and
//! database spelling is a single string per variant, with `ALL`, `as_str`, `Display`,
//! `FromStr`, and string-based `Serialize`/`Deserialize` that can never disagree.

/// Declares a string-backed enum.
///
/// ```text
/// string_enum! {
///     /// Docs.
///     pub enum Lang("language") {
///         /// Arabic.
///         Ar => "ar",
///         /// Also accepts an alias when parsing.
///         En => "en" | "english",
///     }
/// }
/// ```
///
/// The first string of each variant is canonical (`as_str`, `Display`, serialisation); any
/// further strings are accepted by `FromStr`/deserialisation only. Parsing is exact
/// (case-sensitive, no trimming).
macro_rules! string_enum {
    (
        $(#[$meta:meta])*
        pub enum $name:ident($what:literal) {
            $(
                $(#[$vmeta:meta])*
                $variant:ident => $canonical:literal $(| $alias:literal)*
            ),+ $(,)?
        }
    ) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum $name {
            $(
                $(#[$vmeta])*
                $variant,
            )+
        }

        impl $name {
            /// Every variant, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            /// The canonical string of every variant, in declaration order.
            pub const NAMES: &'static [&'static str] = &[$($canonical),+];

            /// The canonical string (wire, frontmatter and database spelling).
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $canonical,)+
                }
            }
        }

        impl ::std::fmt::Display for $name {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl ::std::str::FromStr for $name {
            type Err = $crate::ParseError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($canonical $(| $alias)* => Ok(Self::$variant),)+
                    _ => Err($crate::ParseError::new($what, s)),
                }
            }
        }

        impl ::serde::Serialize for $name {
            fn serialize<S: ::serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(self.as_str())
            }
        }

        impl<'de> ::serde::Deserialize<'de> for $name {
            fn deserialize<D: ::serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct Visitor;

                impl ::serde::de::Visitor<'_> for Visitor {
                    type Value = $name;

                    fn expecting(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                        write!(f, "a {} string", $what)
                    }

                    fn visit_str<E: ::serde::de::Error>(self, v: &str) -> Result<$name, E> {
                        v.parse().map_err(|_| E::unknown_variant(v, $name::NAMES))
                    }

                    fn visit_bytes<E: ::serde::de::Error>(self, v: &[u8]) -> Result<$name, E> {
                        let s = ::std::str::from_utf8(v).map_err(|_| {
                            E::invalid_value(::serde::de::Unexpected::Bytes(v), &self)
                        })?;
                        self.visit_str(s)
                    }
                }

                deserializer.deserialize_str(Visitor)
            }
        }
    };
}

pub(crate) use string_enum;
