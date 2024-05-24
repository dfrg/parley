// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use crate::inline_box::InlineBox;
use crate::layout::{Alignment, Glyph, LineMetrics, RunMetrics, Style};
use crate::style::Brush;
use crate::util::*;
use crate::Font;
use core::ops::Range;
use swash::shape::Shaper;
use swash::text::cluster::{Boundary, ClusterInfo};
use swash::Synthesis;

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

#[derive(Copy, Clone)]
pub struct ClusterData {
    pub info: ClusterInfo,
    pub flags: u16,
    pub style_index: u16,
    pub glyph_len: u8,
    pub text_len: u8,
    /// If `glyph_len == 0xFF`, then `glyph_offset` is a glyph identifier,
    /// otherwise, it's an offset into the glyph array with the base
    /// taken from the owning run.
    pub glyph_offset: u16,
    pub text_offset: u16,
    pub advance: f32,
}

impl ClusterData {
    pub const LIGATURE_START: u16 = 1;
    pub const LIGATURE_COMPONENT: u16 = 2;
    pub const DIVERGENT_STYLES: u16 = 4;

    pub fn is_ligature_start(self) -> bool {
        self.flags & Self::LIGATURE_START != 0
    }

    pub fn is_ligature_component(self) -> bool {
        self.flags & Self::LIGATURE_COMPONENT != 0
    }

    pub fn has_divergent_styles(self) -> bool {
        self.flags & Self::DIVERGENT_STYLES != 0
    }

    pub fn text_range(self, run: &RunData) -> Range<usize> {
        let start = run.text_range.start + self.text_offset as usize;
        start..start + self.text_len as usize
    }
}

#[derive(Clone)]
pub struct RunData {
    /// Index of the font for the run.
    pub font_index: usize,
    /// Font size.
    pub font_size: f32,
    /// Synthesis information for the font.
    pub synthesis: Synthesis,
    /// Range of normalized coordinates in the layout data.
    pub coords_range: Range<usize>,
    /// Range of the source text.
    pub text_range: Range<usize>,
    /// Bidi level for the run.
    pub bidi_level: u8,
    /// True if the run ends with a newline.
    pub ends_with_newline: bool,
    /// Range of clusters.
    pub cluster_range: Range<usize>,
    /// Base for glyph indices.
    pub glyph_start: usize,
    /// Metrics for the run.
    pub metrics: RunMetrics,
    /// Additional word spacing.
    pub word_spacing: f32,
    /// Additional letter spacing.
    pub letter_spacing: f32,
    /// Total advance of the run.
    pub advance: f32,
}

#[derive(Copy, Clone, PartialEq)]
pub enum BreakReason {
    None,
    Regular,
    Explicit,
    Emergency,
}

impl Default for BreakReason {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone, Default)]
pub struct LineData {
    /// Range of the source text.
    pub text_range: Range<usize>,
    /// Range of line runs.
    pub run_range: Range<usize>,
    /// Metrics for the line.
    pub metrics: LineMetrics,
    /// The cause of the line break.
    pub break_reason: BreakReason,
    /// Alignment.
    pub alignment: Alignment,
    /// Maximum advance for the line.
    pub max_advance: f32,
    /// Number of justified clusters on the line.
    pub num_spaces: usize,
}

impl LineData {
    pub fn size(&self) -> f32 {
        self.metrics.ascent + self.metrics.descent + self.metrics.leading
    }
}

#[derive(Clone, Default)]
pub struct LineRunData {
    /// Index of the original run.
    pub run_index: usize,
    /// Bidi level for the run.
    pub bidi_level: u8,
    /// True if the run is composed entirely of whitespace.
    pub is_whitespace: bool,
    /// True if the run ends in whitespace.
    pub has_trailing_whitespace: bool,
    /// Range of the source text.
    pub text_range: Range<usize>,
    /// Range of clusters.
    pub cluster_range: Range<usize>,
    /// Advance for the run.
    pub advance: f32,
}

impl LineRunData {
    pub fn compute_line_height<B: Brush>(&self, layout: &LayoutData<B>) -> f32 {
        let mut line_height = 0f32;
        let glyph_start = layout.runs[self.run_index].glyph_start;
        for cluster in &layout.clusters[self.cluster_range.clone()] {
            if cluster.glyph_len != 0xFF && cluster.has_divergent_styles() {
                let start = glyph_start + cluster.glyph_offset as usize;
                let end = start + cluster.glyph_len as usize;
                for glyph in &layout.glyphs[start..end] {
                    line_height = line_height.max(layout.styles[glyph.style_index()].line_height);
                }
            } else {
                line_height =
                    line_height.max(layout.styles[cluster.style_index as usize].line_height);
            }
        }
        line_height
    }
}

#[derive(Debug, Clone)]
pub enum LayoutItemKind {
    TextRun,
    InlineBox,
}

#[derive(Debug, Clone)]
pub struct LayoutItem {
    /// Whether the item is a run or an inline box
    pub kind: LayoutItemKind,
    /// The index of the run or inline box in the runs or inline_boxes vec
    pub index: usize,
}

#[derive(Clone)]
pub struct LayoutData<B: Brush> {
    pub scale: f32,
    pub has_bidi: bool,
    pub base_level: u8,
    pub text_len: usize,
    pub width: f32,
    pub full_width: f32,
    pub height: f32,
    pub fonts: Vec<Font>,
    pub coords: Vec<i16>,
    pub styles: Vec<Style<B>>,
    pub inline_boxes: Vec<InlineBox>,
    pub runs: Vec<RunData>,
    pub items: Vec<LayoutItem>,
    pub clusters: Vec<ClusterData>,
    pub glyphs: Vec<Glyph>,
    pub lines: Vec<LineData>,
    pub line_runs: Vec<LineRunData>,
}

impl<B: Brush> Default for LayoutData<B> {
    fn default() -> Self {
        Self {
            scale: 1.,
            has_bidi: false,
            base_level: 0,
            text_len: 0,
            width: 0.,
            full_width: 0.,
            height: 0.,
            fonts: Vec::new(),
            coords: Vec::new(),
            styles: Vec::new(),
            inline_boxes: Vec::new(),
            runs: Vec::new(),
            items: Vec::new(),
            clusters: Vec::new(),
            glyphs: Vec::new(),
            lines: Vec::new(),
            line_runs: Vec::new(),
        }
    }
}

impl<B: Brush> LayoutData<B> {
    pub fn clear(&mut self) {
        self.scale = 1.;
        self.has_bidi = false;
        self.base_level = 0;
        self.text_len = 0;
        self.width = 0.;
        self.full_width = 0.;
        self.height = 0.;
        self.fonts.clear();
        self.coords.clear();
        self.styles.clear();
        self.inline_boxes.clear();
        self.runs.clear();
        self.items.clear();
        self.clusters.clear();
        self.glyphs.clear();
        self.lines.clear();
        self.line_runs.clear();
    }

    /// Push an inline box to the list of items
    pub fn push_inline_box(&mut self, index: usize) {
        self.items.push(LayoutItem {
            kind: LayoutItemKind::InlineBox,
            index,
        });
    }

    #[allow(unused_assignments)]
    #[allow(clippy::too_many_arguments)]
    pub fn push_run(
        &mut self,
        font: Font,
        font_size: f32,
        synthesis: Synthesis,
        shaper: Shaper,
        bidi_level: u8,
        word_spacing: f32,
        letter_spacing: f32,
    ) {
        let font_index = self
            .fonts
            .iter()
            .position(|f| *f == font)
            .unwrap_or_else(|| {
                let index = self.fonts.len();
                self.fonts.push(font);
                index
            });
        let metrics = shaper.metrics();
        let cluster_range = self.clusters.len()..self.clusters.len();
        let coords_start = self.coords.len();
        let coords = shaper.normalized_coords();
        if coords.iter().any(|coord| *coord != 0) {
            self.coords.extend_from_slice(coords);
        }
        let coords_end = self.coords.len();
        let mut run = RunData {
            font_index,
            font_size,
            synthesis,
            coords_range: coords_start..coords_end,
            text_range: 0..0,
            bidi_level,
            ends_with_newline: false,
            cluster_range,
            glyph_start: self.glyphs.len(),
            metrics: RunMetrics {
                ascent: metrics.ascent,
                descent: metrics.descent,
                leading: metrics.leading,
                underline_offset: metrics.underline_offset,
                underline_size: metrics.stroke_size,
                strikethrough_offset: metrics.strikeout_offset,
                strikethrough_size: metrics.stroke_size,
            },
            word_spacing,
            letter_spacing,
            advance: 0.,
        };
        // Track these so that we can flush if they overflow a u16.
        let mut glyph_count = 0usize;
        let mut text_offset = 0;
        macro_rules! flush_run {
            () => {
                if !run.cluster_range.is_empty() {
                    self.runs.push(run.clone());
                    self.items.push(LayoutItem {
                        kind: LayoutItemKind::TextRun,
                        index: self.runs.len() - 1,
                    });
                    run.text_range = text_offset..text_offset;
                    run.cluster_range.start = run.cluster_range.end;
                    run.glyph_start = self.glyphs.len();
                    run.advance = 0.;
                    glyph_count = 0;
                }
            };
        }
        let mut first = true;
        shaper.shape_with(|cluster| {
            if cluster.info.boundary() == Boundary::Mandatory {
                run.ends_with_newline = true;
                flush_run!();
            }
            run.ends_with_newline = false;
            const MAX_LEN: usize = u16::MAX as usize;
            let source_range = cluster.source.to_range();
            if first {
                run.text_range = source_range.start..source_range.start;
                text_offset = source_range.start;
                first = false;
            }
            let num_components = cluster.components.len() + 1;
            if glyph_count > MAX_LEN
                || (text_offset - run.text_range.start) > MAX_LEN
                || (num_components > 1
                    && (cluster.components.last().unwrap().start as usize - run.text_range.start)
                        > MAX_LEN)
            {
                flush_run!();
            }
            let text_len = source_range.len();
            let glyph_len = cluster.glyphs.len();
            let advance = cluster.advance();
            run.advance += advance;
            let mut cluster_data = ClusterData {
                info: cluster.info,
                flags: 0,
                style_index: cluster.data as _,
                glyph_len: glyph_len as u8,
                text_len: text_len as u8,
                advance,
                text_offset: (text_offset - run.text_range.start) as u16,
                glyph_offset: 0,
            };
            if num_components > 1 {
                cluster_data.flags = ClusterData::LIGATURE_START;
                cluster_data.advance /= cluster.components.len() as f32;
                cluster_data.text_len = cluster.components[0].to_range().len() as u8;
            }
            macro_rules! push_components {
                () => {
                    self.clusters.push(cluster_data);
                    if num_components > 1 {
                        cluster_data.glyph_offset = 0;
                        cluster_data.glyph_len = 0;
                        for component in &cluster.components[1..] {
                            let range = component.to_range();
                            cluster_data.flags = ClusterData::LIGATURE_COMPONENT;
                            cluster_data.text_offset = (range.start - run.text_range.start) as u16;
                            cluster_data.text_len = range.len() as u8;
                            self.clusters.push(cluster_data);
                            run.cluster_range.end += 1;
                        }
                        cluster_data.flags = 0;
                    }
                };
            }
            run.cluster_range.end += 1;
            run.text_range.end += text_len;
            text_offset += text_len;
            if glyph_len == 1 && num_components == 1 {
                let g = &cluster.glyphs[0];
                if nearly_zero(g.x) && nearly_zero(g.y) {
                    // Handle the case with a single glyph with zero'd offset.
                    cluster_data.glyph_len = 0xFF;
                    cluster_data.glyph_offset = g.id;
                    push_components!();
                    return;
                }
            } else if glyph_len == 0 {
                // Insert an empty cluster. This occurs for both invisible
                // control characters and ligature components.
                push_components!();
                return;
            }
            // Otherwise, encode all of the glyphs.
            cluster_data.glyph_offset = (self.glyphs.len() - run.glyph_start) as u16;
            self.glyphs.extend(cluster.glyphs.iter().map(|g| {
                let style_index = g.data as u16;
                if cluster_data.style_index != style_index {
                    cluster_data.flags |= ClusterData::DIVERGENT_STYLES;
                }
                Glyph {
                    id: g.id,
                    style_index,
                    x: g.x,
                    y: g.y,
                    advance: g.advance,
                }
            }));
            glyph_count += glyph_len;
            push_components!();
        });
        flush_run!();
    }

    pub fn finish(&mut self) {
        for run in &self.runs {
            let word = run.word_spacing;
            let letter = run.letter_spacing;
            if nearly_zero(word) && nearly_zero(letter) {
                continue;
            }
            let clusters = &mut self.clusters[run.cluster_range.clone()];
            for cluster in clusters {
                let mut spacing = letter;
                if !nearly_zero(word) && cluster.info.whitespace().is_space_or_nbsp() {
                    spacing += word;
                }
                if !nearly_zero(spacing) {
                    cluster.advance += spacing;
                    if cluster.glyph_len != 0xFF {
                        let start = run.glyph_start + cluster.glyph_offset as usize;
                        let end = start + cluster.glyph_len as usize;
                        let glyphs = &mut self.glyphs[start..end];
                        if let Some(last) = glyphs.last_mut() {
                            last.advance += spacing;
                        }
                    }
                }
            }
        }
    }
}
