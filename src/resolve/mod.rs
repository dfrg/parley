//! Resolution of dynamic properties within a context.

pub mod range;
pub mod tree;

use super::style::{
    Brush, FontFamily, FontFeature, FontSettings, FontStack, FontStretch, FontStyle, FontVariation,
    FontWeight, StyleProperty,
};
use crate::font::FontContext;
use crate::fontique::FamilyId;
use crate::util::nearly_eq;
use swash::text::Language;
use swash::Setting;

/// Handle for a managed property.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Resolved<T> {
    index: usize,
    _phantom: core::marker::PhantomData<T>,
}

impl<T> Default for Resolved<T> {
    fn default() -> Self {
        Self {
            index: usize::MAX,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<T> Resolved<T> {
    pub fn id(&self) -> usize {
        self.index
    }
}

#[derive(Clone)]
struct Cache<T> {
    /// Items in the cache. May contain sequences.
    items: Vec<T>,
    /// Each entry represents a range of items in `data`.
    entries: Vec<(usize, usize)>,
}

impl<T> Default for Cache<T> {
    fn default() -> Self {
        Self {
            items: vec![],
            entries: vec![],
        }
    }
}

impl<T: Clone + PartialEq> Cache<T> {
    pub fn clear(&mut self) {
        self.items.clear();
        self.entries.clear();
    }

    pub fn insert(&mut self, items: &[T]) -> Resolved<T> {
        for (i, entry) in self.entries.iter().enumerate() {
            let range = entry.0..entry.1;
            if range.len() != items.len() {
                continue;
            }
            if let Some(existing) = self.items.get(range) {
                if existing == items {
                    return Resolved {
                        index: i,
                        _phantom: core::marker::PhantomData,
                    };
                }
            }
        }
        let index = self.entries.len();
        let start = self.items.len();
        self.items.extend(items.iter().cloned());
        let end = self.items.len();
        self.entries.push((start, end));
        Resolved {
            index,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn get(&self, handle: Resolved<T>) -> Option<&[T]> {
        let (start, end) = *self.entries.get(handle.index)?;
        self.items.get(start..end)
    }
}

/// Context for managing dynamic properties during layout.
#[derive(Clone, Default)]
pub struct ResolveContext {
    families: Cache<FamilyId>,
    variations: Cache<Setting<f32>>,
    features: Cache<Setting<u16>>,
    tmp_families: Vec<FamilyId>,
    tmp_variations: Vec<Setting<f32>>,
    tmp_features: Vec<Setting<u16>>,
}

impl ResolveContext {
    pub fn resolve<B: Brush>(
        &mut self,
        fcx: &mut FontContext,
        property: &StyleProperty<B>,
        scale: f32,
    ) -> ResolvedProperty<B> {
        use ResolvedProperty::*;
        match property {
            StyleProperty::FontStack(value) => FontStack(self.resolve_stack(fcx, *value)),
            StyleProperty::FontSize(value) => FontSize(*value * scale),
            StyleProperty::FontStretch(value) => FontStretch(*value),
            StyleProperty::FontStyle(value) => FontStyle(*value),
            StyleProperty::FontWeight(value) => FontWeight(*value),
            StyleProperty::FontVariations(value) => FontVariations(self.resolve_variations(*value)),
            StyleProperty::FontFeatures(value) => FontFeatures(self.resolve_features(*value)),
            StyleProperty::Locale(value) => Locale(value.map(Language::parse).flatten()),
            StyleProperty::Brush(value) => Brush(value.clone()),
            StyleProperty::Underline(value) => Underline(*value),
            StyleProperty::UnderlineOffset(value) => UnderlineOffset(value.map(|x| x * scale)),
            StyleProperty::UnderlineSize(value) => UnderlineSize(value.map(|x| x * scale)),
            StyleProperty::UnderlineBrush(value) => UnderlineBrush(value.clone()),
            StyleProperty::Strikethrough(value) => Strikethrough(*value),
            StyleProperty::StrikethroughOffset(value) => {
                StrikethroughOffset(value.map(|x| x * scale))
            }
            StyleProperty::StrikethroughSize(value) => StrikethroughSize(value.map(|x| x * scale)),
            StyleProperty::StrikethroughBrush(value) => StrikethroughBrush(value.clone()),
            StyleProperty::LineHeight(value) => LineHeight(*value),
            StyleProperty::WordSpacing(value) => WordSpacing(*value * scale),
            StyleProperty::LetterSpacing(value) => LetterSpacing(*value * scale),
        }
    }

    /// Resolves a font stack.
    pub fn resolve_stack(&mut self, fcx: &mut FontContext, stack: FontStack) -> Resolved<FamilyId> {
        self.tmp_families.clear();
        match stack {
            FontStack::Source(source) => {
                for family in FontFamily::parse_list(source) {
                    match family {
                        FontFamily::Named(name) => {
                            if let Some(family) = fcx.collection.family_by_name(name) {
                                self.tmp_families.push(family.id());
                            }
                        }
                        FontFamily::Generic(family) => {
                            self.tmp_families
                                .extend(fcx.collection.generic_families(family));
                        }
                    }
                }
            }
            FontStack::Single(family) => match family {
                FontFamily::Named(name) => {
                    if let Some(family) = fcx.collection.family_by_name(name) {
                        self.tmp_families.push(family.id());
                    }
                }
                FontFamily::Generic(family) => {
                    self.tmp_families
                        .extend(fcx.collection.generic_families(family));
                }
            },
            FontStack::List(families) => {
                for family in families {
                    match family {
                        FontFamily::Named(name) => {
                            if let Some(family) = fcx.collection.family_by_name(name) {
                                self.tmp_families.push(family.id());
                            }
                        }
                        FontFamily::Generic(family) => {
                            self.tmp_families
                                .extend(fcx.collection.generic_families(*family));
                        }
                    }
                }
            }
        }
        let resolved = self.families.insert(&self.tmp_families);
        self.tmp_families.clear();
        resolved
    }

    /// Resolves font variation settings.
    pub fn resolve_variations(
        &mut self,
        variations: FontSettings<FontVariation>,
    ) -> Resolved<Setting<f32>> {
        match variations {
            FontSettings::Source(source) => {
                self.tmp_variations.clear();
                self.tmp_variations
                    .extend(FontVariation::parse_list(source));
            }
            FontSettings::List(settings) => {
                self.tmp_variations.clear();
                self.tmp_variations.extend_from_slice(settings);
            }
        }
        if self.tmp_variations.is_empty() {
            return Resolved::default();
        }
        self.tmp_variations.sort_by(|a, b| a.tag.cmp(&b.tag));
        let resolved = self.variations.insert(&self.tmp_variations);
        self.tmp_variations.clear();
        resolved
    }

    /// Resolves font feature settings.
    pub fn resolve_features(
        &mut self,
        features: FontSettings<FontFeature>,
    ) -> Resolved<Setting<u16>> {
        match features {
            FontSettings::Source(source) => {
                self.tmp_features.clear();
                self.tmp_features.extend(FontFeature::parse_list(source));
            }
            FontSettings::List(settings) => {
                self.tmp_features.clear();
                self.tmp_features.extend_from_slice(settings);
            }
        }
        if self.tmp_features.is_empty() {
            return Resolved::default();
        }
        self.tmp_features.sort_by(|a, b| a.tag.cmp(&b.tag));
        let resolved = self.features.insert(&self.tmp_features);
        self.tmp_features.clear();
        resolved
    }

    /// Returns the list of font families for the specified handle.
    pub fn stack(&self, stack: Resolved<FamilyId>) -> Option<&[FamilyId]> {
        self.families.get(stack)
    }

    /// Returns the list of font variations for the specified handle.
    pub fn variations(&self, variations: Resolved<Setting<f32>>) -> Option<&[Setting<f32>]> {
        self.variations.get(variations)
    }

    /// Returns the list of font features for the specified handle.
    pub fn features(&self, features: Resolved<Setting<u16>>) -> Option<&[Setting<u16>]> {
        self.features.get(features)
    }

    /// Clears the resources in the context.
    pub fn clear(&mut self) {
        self.families.clear();
        self.variations.clear();
        self.features.clear();
    }
}

/// Style property with resolved resources.
#[derive(Clone, PartialEq)]
pub enum ResolvedProperty<B: Brush> {
    /// Font stack.
    FontStack(Resolved<FamilyId>),
    /// Font size.
    FontSize(f32),
    /// Font stretch.
    FontStretch(FontStretch),
    /// Font style.
    FontStyle(FontStyle),
    /// Font weight.
    FontWeight(FontWeight),
    /// Font variation settings.
    FontVariations(Resolved<Setting<f32>>),
    /// Font feature settings.
    FontFeatures(Resolved<Setting<u16>>),
    /// Locale.
    Locale(Option<Language>),
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

/// Flattened group of style properties.
#[derive(Clone, PartialEq, Debug)]
pub struct ResolvedStyle<B: Brush> {
    /// Font stack.
    pub font_stack: Resolved<FamilyId>,
    /// Font size.
    pub font_size: f32,
    /// Font stretch.
    pub font_stretch: FontStretch,
    /// Font style.
    pub font_style: FontStyle,
    /// Font weight.
    pub font_weight: FontWeight,
    /// Font variation settings.
    pub font_variations: Resolved<Setting<f32>>,
    /// Font feature settings.
    pub font_features: Resolved<Setting<u16>>,
    /// Locale.
    pub locale: Option<Language>,
    /// Brush for rendering text.
    pub brush: B,
    /// Underline decoration.
    pub underline: ResolvedDecoration<B>,
    /// Strikethrough decoration.
    pub strikethrough: ResolvedDecoration<B>,
    /// Line height multiplier.
    pub line_height: f32,
    /// Extra spacing between words.
    pub word_spacing: f32,
    /// Extra spacing between letters.
    pub letter_spacing: f32,
}

impl<B: Brush> Default for ResolvedStyle<B> {
    fn default() -> Self {
        Self {
            font_stack: Resolved::default(),
            font_size: 16.,
            font_stretch: Default::default(),
            font_style: Default::default(),
            font_weight: Default::default(),
            font_variations: Default::default(),
            font_features: Default::default(),
            locale: None,
            brush: Default::default(),
            underline: Default::default(),
            strikethrough: Default::default(),
            line_height: 1.,
            word_spacing: 0.,
            letter_spacing: 0.,
        }
    }
}

impl<B: Brush> ResolvedStyle<B> {
    /// Applies the specified property to this style.
    pub fn apply(&mut self, property: ResolvedProperty<B>) {
        use ResolvedProperty::*;
        match property {
            FontStack(value) => self.font_stack = value,
            FontSize(value) => self.font_size = value,
            FontStretch(value) => self.font_stretch = value,
            FontStyle(value) => self.font_style = value,
            FontWeight(value) => self.font_weight = value,
            FontVariations(value) => self.font_variations = value,
            FontFeatures(value) => self.font_features = value,
            Locale(value) => self.locale = value,
            Brush(value) => self.brush = value,
            Underline(value) => self.underline.enabled = value,
            UnderlineOffset(value) => self.underline.offset = value,
            UnderlineSize(value) => self.underline.size = value,
            UnderlineBrush(value) => self.underline.brush = value,
            Strikethrough(value) => self.strikethrough.enabled = value,
            StrikethroughOffset(value) => self.strikethrough.offset = value,
            StrikethroughSize(value) => self.strikethrough.size = value,
            StrikethroughBrush(value) => self.strikethrough.brush = value,
            LineHeight(value) => self.line_height = value,
            WordSpacing(value) => self.word_spacing = value,
            LetterSpacing(value) => self.letter_spacing = value,
        }
    }

    pub fn check(&self, property: &ResolvedProperty<B>) -> bool {
        use ResolvedProperty::*;
        match property {
            FontStack(value) => self.font_stack == *value,
            FontSize(value) => nearly_eq(self.font_size, *value),
            FontStretch(value) => self.font_stretch == *value,
            FontStyle(value) => self.font_style == *value,
            FontWeight(value) => self.font_weight == *value,
            FontVariations(value) => self.font_variations == *value,
            FontFeatures(value) => self.font_features == *value,
            Locale(value) => self.locale == *value,
            Brush(value) => self.brush == *value,
            Underline(value) => self.underline.enabled == *value,
            UnderlineOffset(value) => self.underline.offset == *value,
            UnderlineSize(value) => self.underline.size == *value,
            UnderlineBrush(value) => self.underline.brush == *value,
            Strikethrough(value) => self.strikethrough.enabled == *value,
            StrikethroughOffset(value) => self.strikethrough.offset == *value,
            StrikethroughSize(value) => self.strikethrough.size == *value,
            StrikethroughBrush(value) => self.strikethrough.brush == *value,
            LineHeight(value) => nearly_eq(self.line_height, *value),
            WordSpacing(value) => nearly_eq(self.word_spacing, *value),
            LetterSpacing(value) => nearly_eq(self.letter_spacing, *value),
        }
    }
}

/// Underline or strikethrough decoration.
#[derive(Clone, PartialEq, Default, Debug)]
pub struct ResolvedDecoration<B: Brush> {
    /// True if the decoration is enabled.
    pub enabled: bool,
    /// Offset of the decoration from the baseline.
    pub offset: Option<f32>,
    /// Thickness of the decoration stroke.
    pub size: Option<f32>,
    /// Brush for the decoration.
    pub brush: Option<B>,
}
