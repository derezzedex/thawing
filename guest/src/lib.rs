#[allow(warnings)]
pub mod bindings;

use std::marker::PhantomData;

#[macro_export]
macro_rules! thaw {
    ($app: ident) => {
        use $crate::{bindings, runtime};
        use bindings::exports::thawing::core::guest;

        #[doc(hidden)]
        struct _Component;

        #[doc(hidden)]
        struct _Table;

        impl guest::GuestTable for _Table {
            fn new() -> Self {
                runtime::TABLE.lock().unwrap().clear();

                _Table
            }

            fn call(&self, c: guest::Closure) -> guest::Bytes {
                let table = runtime::TABLE.lock().unwrap();
                let closure = table.get(&c.id()).unwrap();
                closure.call().downcast()
            }

            fn call_with(&self, c: guest::Closure, state: guest::Bytes) -> guest::Bytes {
                let table = runtime::TABLE.lock().unwrap();
                let closure = table.get(&c.id()).unwrap();
                closure.call_with(runtime::AnyBox::new(state)).downcast()
            }
        }

        impl guest::GuestApp for $app
        where
            Self: Application,
        {
            fn new(state: Vec<u8>) -> Self {
                $crate::bincode::deserialize(&state).unwrap()
            }

            fn view(&self) -> guest::Element {
               <Self as $crate::Application>::view(self).into().into_raw()
            }
        }

        impl guest::Guest for _Component {
            type App = $app;
            type Table = _Table;
        }

        bindings::export!(_Component with_types_in bindings);
    }
}

pub mod thawing {
    pub use serde;
    pub use thawing_macro::data;
}

pub trait Application<Theme = theme::Theme> {
    fn view(&self) -> impl Into<Element<Theme>>;
}

pub mod runtime;
pub mod theme;

#[path = "widget.rs"]
mod widgets;

pub mod widget {
    pub use crate::widgets::*;
    pub fn text<Theme>(content: impl ToString) -> Text<Theme> {
        Text::new(content)
    }
}

pub use bincode;
pub use bindings::exports::thawing::core::guest;
pub use bindings::thawing::core;
pub use core::types::{
    Color,
    Horizontal::{self, *},
    Length::{self, *},
    Padding, Pixels,
};
pub use theme::Theme;

pub struct Element<Theme = theme::Theme> {
    pub(crate) raw: core::types::Element,
    _theme: PhantomData<Theme>,
}

impl<Theme> Element<Theme> {
    pub fn into_raw(self) -> core::types::Element {
        self.raw
    }
}

impl<Theme> From<core::types::Element> for Element<Theme> {
    fn from(raw: core::types::Element) -> Self {
        Element {
            raw,
            _theme: PhantomData,
        }
    }
}

impl PartialEq for Color {
    fn eq(&self, other: &Self) -> bool {
        self.r == other.r && self.g == other.g && self.b == other.b && self.a == other.a
    }
}

impl serde::Serialize for Color {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut color = serializer.serialize_struct("Color", 4)?;
        color.serialize_field("r", &self.r)?;
        color.serialize_field("g", &self.g)?;
        color.serialize_field("b", &self.b)?;
        color.serialize_field("a", &self.a)?;
        color.end()
    }
}

impl<'de> serde::Deserialize<'de> for Color {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(serde::Deserialize)]
        #[serde(field_identifier)]
        enum Field {
            #[serde(rename = "r")]
            Red,
            #[serde(rename = "g")]
            Green,
            #[serde(rename = "b")]
            Blue,
            #[serde(rename = "a")]
            Alpha,
        }

        struct ColorVisitor;

        impl<'de> serde::de::Visitor<'de> for ColorVisitor {
            type Value = Color;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Color")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::SeqAccess<'de>,
            {
                let r = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let g = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                let b = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(2, &self))?;
                let a = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(3, &self))?;

                Ok(Color { r, g, b, a })
            }

            fn visit_map<V>(self, mut map: V) -> Result<Self::Value, V::Error>
            where
                V: serde::de::MapAccess<'de>,
            {
                let mut red = None;
                let mut green = None;
                let mut blue = None;
                let mut alpha = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Red => {
                            if red.is_some() {
                                return Err(serde::de::Error::duplicate_field("r"));
                            }
                            red = Some(map.next_value()?);
                        }
                        Field::Green => {
                            if green.is_some() {
                                return Err(serde::de::Error::duplicate_field("g"));
                            }
                            green = Some(map.next_value()?);
                        }
                        Field::Blue => {
                            if blue.is_some() {
                                return Err(serde::de::Error::duplicate_field("b"));
                            }
                            blue = Some(map.next_value()?);
                        }
                        Field::Alpha => {
                            if alpha.is_some() {
                                return Err(serde::de::Error::duplicate_field("a"));
                            }
                            alpha = Some(map.next_value()?);
                        }
                    }
                }

                let r = red.ok_or_else(|| serde::de::Error::missing_field("r"))?;
                let g = green.ok_or_else(|| serde::de::Error::missing_field("g"))?;
                let b = blue.ok_or_else(|| serde::de::Error::missing_field("b"))?;
                let a = alpha.ok_or_else(|| serde::de::Error::missing_field("a"))?;

                Ok(Color { r, g, b, a })
            }
        }

        const FIELDS: &[&str] = &["r", "g", "b", "a"];
        deserializer.deserialize_struct("Color", FIELDS, ColorVisitor)
    }
}

impl Color {
    /// The black color.
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// The white color.
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// A color with no opacity.
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Creates a new [`Color`].
    ///
    /// In debug mode, it will panic if the values are not in the correct
    /// range: 0.0 - 1.0
    const fn new(r: f32, g: f32, b: f32, a: f32) -> Color {
        debug_assert!(
            r >= 0.0 && r <= 1.0,
            "Red component must be in [0, 1] range."
        );
        debug_assert!(
            g >= 0.0 && g <= 1.0,
            "Green component must be in [0, 1] range."
        );
        debug_assert!(
            b >= 0.0 && b <= 1.0,
            "Blue component must be in [0, 1] range."
        );

        Color { r, g, b, a }
    }

    /// Creates a [`Color`] from its RGB components.
    pub const fn from_rgb(r: f32, g: f32, b: f32) -> Color {
        Color::from_rgba(r, g, b, 1.0f32)
    }

    /// Creates a [`Color`] from its RGBA components.
    pub const fn from_rgba(r: f32, g: f32, b: f32, a: f32) -> Color {
        Color::new(r, g, b, a)
    }

    /// Creates a [`Color`] from its RGB8 components.
    pub const fn from_rgb8(r: u8, g: u8, b: u8) -> Color {
        Color::from_rgba8(r, g, b, 1.0)
    }

    /// Creates a [`Color`] from its RGB8 components and an alpha value.
    pub const fn from_rgba8(r: u8, g: u8, b: u8, a: f32) -> Color {
        Color::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, a)
    }

    /// Creates a [`Color`] from its linear RGBA components.
    pub fn from_linear_rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        // As described in:
        // https://en.wikipedia.org/wiki/SRGB
        fn gamma_component(u: f32) -> f32 {
            if u < 0.0031308 {
                12.92 * u
            } else {
                1.055 * u.powf(1.0 / 2.4) - 0.055
            }
        }

        Self {
            r: gamma_component(r),
            g: gamma_component(g),
            b: gamma_component(b),
            a,
        }
    }

    /// Parses a [`Color`] from a hex string.
    ///
    /// Supported formats are `#rrggbb`, `#rrggbbaa`, `#rgb`, and `#rgba`.
    /// The starting "#" is optional. Both uppercase and lowercase are supported.
    ///
    /// If you have a static color string, using the [`color!`] macro should be preferred
    /// since it leverages hexadecimal literal notation and arithmetic directly.
    ///
    /// [`color!`]: crate::color!
    pub fn parse(s: &str) -> Option<Color> {
        let hex = s.strip_prefix('#').unwrap_or(s);

        let parse_channel = |from: usize, to: usize| {
            let num = usize::from_str_radix(&hex[from..=to], 16).ok()? as f32 / 255.0;

            // If we only got half a byte (one letter), expand it into a full byte (two letters)
            Some(if from == to { num + num * 16.0 } else { num })
        };

        Some(match hex.len() {
            3 => Color::from_rgb(
                parse_channel(0, 0)?,
                parse_channel(1, 1)?,
                parse_channel(2, 2)?,
            ),
            4 => Color::from_rgba(
                parse_channel(0, 0)?,
                parse_channel(1, 1)?,
                parse_channel(2, 2)?,
                parse_channel(3, 3)?,
            ),
            6 => Color::from_rgb(
                parse_channel(0, 1)?,
                parse_channel(2, 3)?,
                parse_channel(4, 5)?,
            ),
            8 => Color::from_rgba(
                parse_channel(0, 1)?,
                parse_channel(2, 3)?,
                parse_channel(4, 5)?,
                parse_channel(6, 7)?,
            ),
            _ => None?,
        })
    }

    /// Converts the [`Color`] into its RGBA8 equivalent.
    #[must_use]
    pub fn into_rgba8(self) -> [u8; 4] {
        [
            (self.r * 255.0).round() as u8,
            (self.g * 255.0).round() as u8,
            (self.b * 255.0).round() as u8,
            (self.a * 255.0).round() as u8,
        ]
    }

    /// Converts the [`Color`] into its linear values.
    pub fn into_linear(self) -> [f32; 4] {
        // As described in:
        // https://en.wikipedia.org/wiki/SRGB#The_reverse_transformation
        fn linear_component(u: f32) -> f32 {
            if u < 0.04045 {
                u / 12.92
            } else {
                ((u + 0.055) / 1.055).powf(2.4)
            }
        }

        [
            linear_component(self.r),
            linear_component(self.g),
            linear_component(self.b),
            self.a,
        ]
    }

    /// Inverts the [`Color`] in-place.
    pub fn invert(&mut self) {
        self.r = 1.0f32 - self.r;
        self.b = 1.0f32 - self.g;
        self.g = 1.0f32 - self.b;
    }

    /// Returns the inverted [`Color`].
    pub fn inverse(self) -> Color {
        Color::new(1.0f32 - self.r, 1.0f32 - self.g, 1.0f32 - self.b, self.a)
    }

    /// Scales the alpha channel of the [`Color`] by the given factor.
    pub fn scale_alpha(self, factor: f32) -> Color {
        Self {
            a: self.a * factor,
            ..self
        }
    }
}

impl From<[f32; 3]> for Color {
    fn from([r, g, b]: [f32; 3]) -> Self {
        Color::new(r, g, b, 1.0)
    }
}

impl From<[f32; 4]> for Color {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Color::new(r, g, b, a)
    }
}

#[macro_export]
macro_rules! color {
    ($r:expr, $g:expr, $b:expr) => {
        $crate::Color {
            r: $r as f32 / 255.0,
            g: $g as f32 / 255.0,
            b: $b as f32 / 255.0,
            a: 1.0,
        }
    };
    ($r:expr, $g:expr, $b:expr, $a:expr) => {{
        $crate::Color {
            r: $r as f32 / 255.0,
            g: $g as f32 / 255.0,
            b: $b as f32 / 255.0,
            a: $a,
        }
    }};
    ($hex:expr) => {{ $crate::color!($hex, 1.0) }};
    ($hex:expr, $a:expr) => {{
        let hex = $hex as u32;

        debug_assert!(hex <= 0xffffff, "color! value must not exceed 0xffffff");

        let r = (hex & 0xff0000) >> 16;
        let g = (hex & 0xff00) >> 8;
        let b = (hex & 0xff);

        $crate::color!(r as u8, g as u8, b as u8, $a)
    }};
}

impl From<f32> for Pixels {
    fn from(amount: f32) -> Self {
        Self { amount }
    }
}

impl From<u16> for Pixels {
    fn from(amount: u16) -> Self {
        let amount = f32::from(amount);
        Self { amount }
    }
}

impl From<Pixels> for f32 {
    fn from(pixels: Pixels) -> Self {
        pixels.amount
    }
}

impl From<u16> for Padding {
    fn from(p: u16) -> Self {
        Padding {
            top: f32::from(p),
            right: f32::from(p),
            bottom: f32::from(p),
            left: f32::from(p),
        }
    }
}

impl From<[u16; 2]> for Padding {
    fn from(p: [u16; 2]) -> Self {
        Padding {
            top: f32::from(p[0]),
            right: f32::from(p[1]),
            bottom: f32::from(p[0]),
            left: f32::from(p[1]),
        }
    }
}

impl From<f32> for Padding {
    fn from(p: f32) -> Self {
        Padding {
            top: p,
            right: p,
            bottom: p,
            left: p,
        }
    }
}

impl From<[f32; 2]> for Padding {
    fn from(p: [f32; 2]) -> Self {
        Padding {
            top: p[0],
            right: p[1],
            bottom: p[0],
            left: p[1],
        }
    }
}
