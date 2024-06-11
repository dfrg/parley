// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Context for layout.

#[cfg(not(feature = "std"))]
use alloc::{vec, vec::Vec};
use fontique::FamilyId;

use super::context::*;
use super::resolve::*;
use super::style::*;
use super::FontContext;

#[cfg(feature = "std")]
use super::layout::{Decoration, Layout, Style};

use core::ops::RangeBounds;

use crate::inline_box::InlineBox;

/// Builder for constructing a text layout with ranged attributes.
pub struct RangedBuilder<'a, B: Brush> {
    pub(crate) scale: f32,
    pub(crate) lcx: &'a mut LayoutContext<B>,
    pub(crate) fcx: &'a mut FontContext,
}

impl<'a, B: Brush> RangedBuilder<'a, B> {
    pub fn push_default(&mut self, property: &StyleProperty<B>) {
        let resolved = self
            .lcx
            .rcx
            .resolve_property(self.fcx, property, self.scale);
        self.lcx.ranged_style_builder.push_default(resolved);
    }

    pub fn push(&mut self, property: &StyleProperty<B>, range: impl RangeBounds<usize>) {
        let resolved = self
            .lcx
            .rcx
            .resolve_property(self.fcx, property, self.scale);
        self.lcx.ranged_style_builder.push(resolved, range);
    }

    pub fn push_inline_box(&mut self, inline_box: InlineBox) {
        self.lcx.inline_boxes.push(inline_box);
    }

    #[cfg(feature = "std")]
    pub fn build_into(&mut self, layout: &mut Layout<B>, text: impl AsRef<str>) {
        // Apply RangedStyleBuilder styles to LayoutContext
        self.lcx.ranged_style_builder.finish(&mut self.lcx.styles);

        // Call generic layout builder method
        build_into_layout(layout, self.scale, text.as_ref(), self.lcx, self.fcx)
    }

    #[cfg(feature = "std")]
    pub fn build(&mut self, text: impl AsRef<str>) -> Layout<B> {
        let mut layout = Layout::default();
        self.build_into(&mut layout, text);
        layout
    }
}

/// Builder for constructing a text layout with a tree of attributes.
pub struct TreeBuilder<'a, B: Brush> {
    pub(crate) scale: f32,
    pub(crate) lcx: &'a mut LayoutContext<B>,
    pub(crate) fcx: &'a mut FontContext,
}

impl<'a, B: Brush> TreeBuilder<'a, B> {
    pub fn push_style_span(&mut self, style: TextStyle<B>) {
        let resolved = self
            .lcx
            .rcx
            .resolve_entire_style_set(self.fcx, &style, self.scale);
        self.lcx.tree_style_builder.push_style_span(resolved);
    }

    pub fn push_style_modification_span<'s, 'iter>(
        &mut self,
        properties: impl IntoIterator<Item = &'iter StyleProperty<'s, B>>,
    ) where
        's: 'iter,
        B: 'iter,
    {
        self.lcx.tree_style_builder.push_style_modification_span(
            properties
                .into_iter()
                .map(|p| self.lcx.rcx.resolve_property(self.fcx, p, self.scale)),
        )
    }

    pub fn pop_style_span(&mut self) {
        self.lcx.tree_style_builder.pop_style_span();
    }

    pub fn push_text(&mut self, len: usize) {
        self.lcx.tree_style_builder.push_text(len);
    }

    pub fn push_inline_box(&mut self, inline_box: InlineBox) {
        self.lcx.inline_boxes.push(inline_box);
    }

    #[cfg(feature = "std")]
    pub fn build_into(&mut self, layout: &mut Layout<B>, text: impl AsRef<str>) {
        let text = text.as_ref();

        // Apply TreeStyleBuilder styles to LayoutContext
        self.lcx.tree_style_builder.finish(&mut self.lcx.styles);

        self.lcx.analyze_text(text);

        // Call generic layout builder method
        build_into_layout(layout, self.scale, text, self.lcx, self.fcx)
    }

    #[cfg(feature = "std")]
    pub fn build(&mut self, text: impl AsRef<str>) -> Layout<B> {
        let mut layout = Layout::default();
        self.build_into(&mut layout, text);
        layout
    }
}

fn build_into_layout<B: Brush>(
    layout: &mut Layout<B>,
    scale: f32,
    text: &str,
    lcx: &mut LayoutContext<B>,
    fcx: &mut FontContext,
) {
    // Force a layout to have at least one line.
    // TODO: support layouts with no text
    let is_empty = text.is_empty();
    let text = if is_empty { " " } else { text };

    layout.data.clear();
    layout.data.scale = scale;
    layout.data.has_bidi = !lcx.bidi.levels().is_empty();
    layout.data.base_level = lcx.bidi.base_level();
    layout.data.text_len = text.len();

    println!("BUILD INTO ({})", text.len());
    for span in &lcx.styles {
        let stack = lcx.rcx.stack(span.style.font_stack);
        println!(
            "{:?} weight:{}, family: {:?}",
            span.range, span.style.font_weight, stack
        );
    }

    let mut char_index = 0;
    for (i, style) in lcx.styles.iter().enumerate() {
        for _ in text[style.range.clone()].chars() {
            lcx.info[char_index].1 = i as u16;
            char_index += 1;
        }
    }

    // Define a function that converts `ResolvedDecoration` into `Decoration` (used just below)
    fn conv_deco<B: Brush>(
        deco: &ResolvedDecoration<B>,
        default_brush: &B,
    ) -> Option<Decoration<B>> {
        if deco.enabled {
            Some(Decoration {
                brush: deco.brush.clone().unwrap_or_else(|| default_brush.clone()),
                offset: deco.offset,
                size: deco.size,
            })
        } else {
            None
        }
    }

    // Copy the visual styles into the layout
    layout.data.styles.extend(lcx.styles.iter().map(|s| {
        let s = &s.style;
        Style {
            brush: s.brush.clone(),
            underline: conv_deco(&s.underline, &s.brush),
            strikethrough: conv_deco(&s.strikethrough, &s.brush),
            line_height: s.line_height,
        }
    }));

    // Sort the inline boxes
    // Note: It's important that this is a stable sort to allow users to control the order of contiguous inline boxes
    lcx.inline_boxes.sort_by_key(|b| b.index);

    // dbg!(&lcx.inline_boxes);

    {
        let query = fcx.collection.query(&mut fcx.source_cache);
        super::shape::shape_text(
            &lcx.rcx,
            query,
            &lcx.styles,
            &lcx.inline_boxes,
            &lcx.info,
            lcx.bidi.levels(),
            &mut lcx.scx,
            text,
            layout,
        );
    }

    // Move inline boxes into the layout
    layout.data.inline_boxes.clear();
    core::mem::swap(&mut layout.data.inline_boxes, &mut lcx.inline_boxes);

    layout.data.finish();

    // Extra processing if the text is empty
    // TODO: update this logic to work with inline boxes
    if is_empty {
        layout.data.text_len = 0;
        if let Some(run) = layout.data.runs.get_mut(0) {
            run.cluster_range.end = 0;
            run.text_range.end = 0;
        }
        layout.data.clusters.clear();
    }
}
