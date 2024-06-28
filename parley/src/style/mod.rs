// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Rich styling support.

mod brush;
mod font;

pub use brush::*;
pub use font::{
    FontFamily, FontFeature, FontSettings, FontStack, FontStretch, FontStyle, FontVariation,
    FontWeight, GenericFamily,
};

#[derive(Debug, Clone, Copy)]
pub enum WhiteSpaceCollapse {
    Collapse,
    Preserve,
}

/// Properties that define a style.
#[derive(Clone, PartialEq, Debug)]
pub enum StyleProperty<'a, B: Brush> {
    /// Font family stack.
    FontStack(FontStack<'a>),
    /// Font size.
    FontSize(f32),
    /// Font stretch.
    FontStretch(FontStretch),
    /// Font style.
    FontStyle(FontStyle),
    /// Font weight.
    FontWeight(FontWeight),
    /// Font variation settings.
    FontVariations(FontSettings<'a, FontVariation>),
    /// Font feature settings.
    FontFeatures(FontSettings<'a, FontFeature>),
    /// Locale.
    Locale(Option<&'a str>),
    /// Brush for rendering text.
    Brush(B),
    /// Underline decoration.
    Underline(bool),
    /// Offset of the underline decoration.
    UnderlineOffset(Option<f32>),
    /// Size of the underline decoration.
    UnderlineSize(Option<f32>),
    /// Brush for rendering the underline decoration.
    UnderlineBrush(Option<B>),
    /// Strikethrough decoration.
    Strikethrough(bool),
    /// Offset of the strikethrough decoration.
    StrikethroughOffset(Option<f32>),
    /// Size of the strikethrough decoration.
    StrikethroughSize(Option<f32>),
    /// Brush for rendering the strikethrough decoration.
    StrikethroughBrush(Option<B>),
    /// Line height multiplier.
    LineHeight(f32),
    /// Extra spacing between words.
    WordSpacing(f32),
    /// Extra spacing between letters.
    LetterSpacing(f32),
}

/// Unresolved styles.
#[derive(Clone, PartialEq, Debug)]
pub struct TextStyle<'a, B: Brush> {
    /// Font family stack.
    pub font_stack: FontStack<'a>,
    /// Font size.
    pub font_size: f32,
    /// Font stretch.
    pub font_stretch: FontStretch,
    /// Font style.
    pub font_style: FontStyle,
    /// Font weight.
    pub font_weight: FontWeight,
    /// Font variation settings.
    pub font_variations: FontSettings<'a, FontVariation>,
    /// Font feature settings.
    pub font_features: FontSettings<'a, FontFeature>,
    /// Locale.
    pub locale: Option<&'a str>,
    /// Brush for rendering text.
    pub brush: B,
    /// Underline decoration.
    pub has_underline: bool,
    /// Offset of the underline decoration.
    pub underline_offset: Option<f32>,
    /// Size of the underline decoration.
    pub underline_size: Option<f32>,
    /// Brush for rendering the underline decoration.
    pub underline_brush: Option<B>,
    /// Strikethrough decoration.
    pub has_strikethrough: bool,
    /// Offset of the strikethrough decoration.
    pub strikethrough_offset: Option<f32>,
    /// Size of the strikethrough decoration.
    pub strikethrough_size: Option<f32>,
    /// Brush for rendering the strikethrough decoration.
    pub strikethrough_brush: Option<B>,
    /// Line height multiplier.
    pub line_height: f32,
    /// Extra spacing between words.
    pub word_spacing: f32,
    /// Extra spacing between letters.
    pub letter_spacing: f32,
}

impl<'a, B: Brush> Default for TextStyle<'a, B> {
    fn default() -> Self {
        TextStyle {
            font_stack: FontStack::Source("sans-serif"),
            font_size: 16.0,
            font_stretch: Default::default(),
            font_style: Default::default(),
            font_weight: Default::default(),
            font_variations: FontSettings::List(&[]),
            font_features: FontSettings::List(&[]),
            locale: Default::default(),
            brush: Default::default(),
            has_underline: Default::default(),
            underline_offset: Default::default(),
            underline_size: Default::default(),
            underline_brush: Default::default(),
            has_strikethrough: Default::default(),
            strikethrough_offset: Default::default(),
            strikethrough_size: Default::default(),
            strikethrough_brush: Default::default(),
            line_height: 1.2,
            word_spacing: Default::default(),
            letter_spacing: Default::default(),
        }
    }
}
