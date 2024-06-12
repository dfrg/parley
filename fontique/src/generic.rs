// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Generic font families.

use super::FamilyId;
use core::fmt;
use smallvec::SmallVec;

type FamilyVec = SmallVec<[FamilyId; 2]>;

/// Describes a generic font family.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum GenericFamily {
    /// Glyphs have finishing strokes, flared or tapering ends, or have actual
    ///  serifed endings.
    Serif = 0,
    /// Glyphs have stroke endings that are plain.
    SansSerif = 1,
    /// All glyphs have the same fixed width.
    Monospace = 2,
    /// Glyphs in cursive fonts generally have either joining strokes or other
    /// cursive characteristics beyond those of italic typefaces. The glyphs
    /// are partially or completely connected, and the result looks more like
    /// handwritten pen or brush writing than printed letter work.
    Cursive = 3,
    /// Fantasy fonts are primarily decorative fonts that contain playful
    /// representations of characters
    Fantasy = 4,
    /// Glyphs are taken from the default user interface font on a given
    /// platform.
    SystemUi = 5,
    /// The default user interface serif font.
    UiSerif = 6,
    /// The default user interface sans-serif font.
    UiSansSerif = 7,
    /// The default user interface monospace font.
    UiMonospace = 8,
    /// The default user interface font that has rounded features.
    UiRounded = 9,
    /// Fonts that are specifically designed to render emoji.
    Emoji = 10,
    /// This is for the particular stylistic concerns of representing
    /// mathematics: superscript and subscript, brackets that cross several
    /// lines, nesting expressions, and double struck glyphs with distinct
    /// meanings.
    Math = 11,
    /// A particular style of Chinese characters that are between serif-style
    /// Song and cursive-style Kai forms. This style is often used for
    /// government documents.
    FangSong = 12,
}

impl GenericFamily {
    /// Parses a generic family from a CSS generic family name.
    ///
    /// # Example
    /// ```
    /// # use fontique::GenericFamily;
    /// assert_eq!(GenericFamily::parse("sans-serif"), Some(GenericFamily::SansSerif));
    /// assert_eq!(GenericFamily::parse("Arial"), None);
    /// ```
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim();
        Some(match s {
            "serif" => Self::Serif,
            "sans-serif" => Self::SansSerif,
            "monospace" => Self::Monospace,
            "cursive" => Self::Cursive,
            "fantasy" => Self::Fantasy,
            "system-ui" => Self::SystemUi,
            "ui-serif" => Self::UiSerif,
            "ui-sans-serif" => Self::UiSansSerif,
            "ui-monospace" => Self::UiMonospace,
            "ui-rounded" => Self::UiRounded,
            "emoji" => Self::Emoji,
            "math" => Self::Math,
            "fangsong" => Self::FangSong,

            // Legacy web compatibility
            #[cfg(target_vendor = "apple")]
            "-apple-system" => Self::SystemUi,
            #[cfg(target_os = "macos")]
            "BlinkMacSystemFont" => Self::SystemUi,

            _ => return None,
        })
    }

    /// Returns a slice containing all generic family variants.
    pub const fn all() -> &'static [GenericFamily] {
        &[
            GenericFamily::SansSerif,
            GenericFamily::Serif,
            GenericFamily::Monospace,
            GenericFamily::Cursive,
            GenericFamily::Fantasy,
            GenericFamily::SystemUi,
            GenericFamily::UiSerif,
            GenericFamily::UiSansSerif,
            GenericFamily::UiMonospace,
            GenericFamily::UiRounded,
            GenericFamily::Emoji,
            GenericFamily::Math,
            GenericFamily::FangSong,
        ]
    }
}

impl fmt::Display for GenericFamily {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let name = match self {
            Self::Serif => "serif",
            Self::SansSerif => "sans-serif",
            Self::Monospace => "monospace",
            Self::Cursive => "cursive",
            Self::Fantasy => "fantasy",
            Self::SystemUi => "system-ui",
            Self::UiSerif => "ui-serif",
            Self::UiSansSerif => "ui-sans-serif",
            Self::UiMonospace => "ui-monospace",
            Self::UiRounded => "ui-rounded",
            Self::Emoji => "emoji",
            Self::Math => "math",
            Self::FangSong => "fangsong",
        };
        write!(f, "{}", name)
    }
}

const COUNT: usize = GenericFamily::FangSong as usize + 1;

/// Maps generic families to family identifiers.
#[derive(Clone, Default, Debug)]
pub struct GenericFamilyMap {
    map: [FamilyVec; COUNT],
}

impl GenericFamilyMap {
    /// Returns the associated family identifiers for the given generic family.
    pub fn get(&self, generic: GenericFamily) -> &[FamilyId] {
        &self.map[generic as usize]
    }

    /// Sets the associated family identifiers for the given generic family.
    pub fn set(&mut self, generic: GenericFamily, families: impl Iterator<Item = FamilyId>) {
        let map = &mut self.map[generic as usize];
        map.clear();
        map.extend(families);
    }

    /// Appends the family identifiers to the list for the given generic family.
    pub fn append(&mut self, generic: GenericFamily, families: impl Iterator<Item = FamilyId>) {
        self.map[generic as usize].extend(families);
    }
}
