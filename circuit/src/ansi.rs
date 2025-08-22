#![allow(dead_code)]

#[derive(Debug, Default, Copy, Clone)]
pub struct AnsiStyle {
    colour: AnsiColour,
    attributes: AnsiAttributes,
}

impl AnsiStyle {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn colour(self, fg: AnsiColourType, bg: AnsiColourType) -> Self {
        Self {
            colour: AnsiColour::colour(fg, bg),
            attributes: self.attributes,
        }
    }

    pub fn fg(self, fg: AnsiColourType) -> Self {
        Self {
            colour: AnsiColour {
                fg: Some(fg),
                bg: self.colour.bg,
            },
            attributes: self.attributes,
        }
    }

    pub fn bg(self, bg: AnsiColourType) -> Self {
        Self {
            colour: AnsiColour {
                fg: self.colour.fg,
                bg: Some(bg),
            },
            attributes: self.attributes,
        }
    }

    pub fn inverse(self) -> Self {
        Self {
            colour: AnsiColour::RESET,
            attributes: self.attributes.inverse(),
        }
    }
}

impl core::fmt::Display for AnsiStyle {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}{}", self.colour, self.attributes)
    }
}

macro_rules! ansi_attributes {
    (
        $(
            $n:ident($set:literal, $unset:literal)
        ),
        *$(,)?
    ) =>{
        #[derive(Debug, Default, Clone, Copy)]
        pub struct AnsiAttributes {
            $($n: Option<bool>),*
        }

        #[allow(dead_code)]
        impl AnsiAttributes {
            const RESET: Self = Self {$($n: Some(false)),*};

            $(
                pub fn $n(mut self, value: bool) -> Self {
                    self.$n = Some(value);
                    self
                }
            )*

            pub fn inverse(&self) -> Self {
                Self {
                    $(
                        $n: self.$n.and_then(|v| Some(!v))
                    ),*
                }
            }
        }

        impl core::fmt::Display for AnsiAttributes {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                $(
                    match self.$n {
                        Some(true) => write!(f, "\x1b[{}m", $set)?,
                        Some(false) => write!(f, "\x1b[{}m", $unset)?,
                        None => {},
                    };
                )*

                Ok(())
            }
        }

        impl AnsiStyle {
            $(
                pub fn $n(self, value: bool) -> Self {
                    Self {
                        colour: self.colour,
                        attributes: self.attributes.$n(value),
                    }
                }
            )*
        }
    };
}

ansi_attributes! {
    bold("1", "22"),
    faint("2", "22"),
    italic("3", "23"),
    underline("4", "24"),
    blink("5", "25"),
    strike("9", "29"),
}

#[derive(Debug, Copy, Clone)]
pub enum AnsiColourType {
    // _4Bit(bright: bool, colour: u8) - colour must be 0-7
    _4Bit(bool, u8),

    // _8Bit(colour: u8)
    _8Bit(u8),

    // _24Bit(r: u8, g: u8, b: u8)
    _24Bit(u8, u8, u8),

    Reset,
}

impl AnsiColourType {
    fn get_prefix(&self, fg: bool) -> &'static str {
        match self {
            Self::_4Bit(bright, _) => {
                if fg {
                    if *bright {
                        "\x1b[3"
                    } else {
                        "\x1b[9"
                    }
                } else if *bright {
                    "\x1b[4"
                } else {
                    "\x1b[10"
                }
            }
            Self::_8Bit(_) => {
                if fg {
                    "\x1b[38;5;"
                } else {
                    "\x1b[48;5;"
                }
            }
            Self::_24Bit(_, _, _) => {
                if fg {
                    "\x1b[38;2;"
                } else {
                    "\x1b[48;2;"
                }
            }
            Self::Reset => {
                if fg {
                    "\x1b[39m"
                } else {
                    "\x1b[49m"
                }
            }
        }
    }

    fn apply(&self, f: &mut core::fmt::Formatter<'_>, fg: bool) -> core::fmt::Result {
        match self {
            Self::_4Bit(_, colour) => write!(f, "{}{}m", self.get_prefix(fg), colour),
            Self::_8Bit(colour) => write!(f, "{}{}m", self.get_prefix(fg), colour),
            Self::_24Bit(r, g, b) => write!(f, "{}{};{};{}m", self.get_prefix(fg), r, g, b),
            Self::Reset => write!(f, "{}", self.get_prefix(fg)),
        }
    }
}

pub const BLACK: AnsiColourType = colour!(0);
pub const RED: AnsiColourType = colour!(1);
pub const GREEN: AnsiColourType = colour!(2);
pub const YELLOW: AnsiColourType = colour!(3);
pub const BLUE: AnsiColourType = colour!(4);
pub const MAGENTA: AnsiColourType = colour!(5);
pub const CYAN: AnsiColourType = colour!(6);
pub const WHITE: AnsiColourType = colour!(7);
pub const RESET: AnsiColourType = colour!(@RESET);

#[derive(Debug, Clone, Copy, Default)]
pub struct AnsiColour {
    fg: Option<AnsiColourType>,
    bg: Option<AnsiColourType>,
}

impl AnsiColour {
    const RESET: Self = Self {
        fg: Some(AnsiColourType::Reset),
        bg: Some(AnsiColourType::Reset),
    };

    pub fn colour(fg: AnsiColourType, bg: AnsiColourType) -> Self {
        Self {
            fg: Some(fg),
            bg: Some(bg),
        }
    }

    pub fn fg(c: AnsiColourType) -> Self {
        Self {
            fg: Some(c),
            bg: None,
        }
    }

    pub fn bg(c: AnsiColourType) -> Self {
        Self {
            fg: None,
            bg: Some(c),
        }
    }
}

impl core::fmt::Display for AnsiColour {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self.fg {
            Some(fg) => fg.apply(f, true)?,
            None => {}
        }

        match self.bg {
            Some(bg) => bg.apply(f, false)?,
            None => {}
        };

        Ok(())
    }
}

impl From<AnsiColourType> for AnsiColour {
    fn from(c: AnsiColourType) -> Self {
        Self::fg(c)
    }
}

impl From<(AnsiColourType, AnsiColourType)> for AnsiColour {
    fn from((fg, bg): (AnsiColourType, AnsiColourType)) -> Self {
        Self::colour(fg, bg)
    }
}

macro_rules! colour {
    (@RESET) => {
        $crate::ansi::AnsiColourType::Reset
    };
    (@BRIGHT $c:expr) => {
        $crate::ansi::AnsiColourType::_4Bit(true, $c)
    };
    ($c:expr) => {
        if $c > 7 {
            $crate::ansi::AnsiColourType::_8Bit($c)
        } else {
            $crate::ansi::AnsiColourType::_4Bit(false, $c)
        }
    };
    ($r:expr, $g:expr, $b:expr) => {
        $crate::ansi::AnsiColourType::_24Bit($r, $g, $b)
    };
}

macro_rules! style {
    ($($style:ident),* $(,)?) => {
        $crate::ansi::AnsiStyle::new()$(.$style(true))*
    };
}

pub struct AnsiText<T> {
    text: T,
    style: AnsiStyle,
}

impl<T: Display> core::fmt::Display for AnsiText<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.style)?;
        f.pad(&format!("{}", self.text))?;
        write!(f, "{}", self.style.inverse())
    }
}

pub trait AnsiModify<T> {
    fn apply(self, style: AnsiStyle) -> AnsiText<T>;

    fn fg(self, fg: AnsiColourType) -> AnsiText<T>
    where
        Self: Sized,
    {
        self.apply(AnsiStyle::new().fg(fg))
    }

    fn bg(self, bg: AnsiColourType) -> AnsiText<T>
    where
        Self: Sized,
    {
        self.apply(AnsiStyle::new().bg(bg))
    }
}

impl<T: Display + Sized> AnsiModify<T> for T {
    fn apply(self, style: AnsiStyle) -> AnsiText<T> {
        AnsiText { text: self, style }
    }
}

pub fn fg(fg: AnsiColourType) -> AnsiColour {
    AnsiColour::fg(fg)
}

pub fn bg(bg: AnsiColourType) -> AnsiColour {
    AnsiColour::bg(bg)
}

use std::fmt::Display;

use colour;
