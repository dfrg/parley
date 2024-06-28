// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use super::*;

pub mod greedy;

impl<'a, B: Brush> Line<'a, B> {
    /// Returns the metrics for the line.
    pub fn metrics(&self) -> &LineMetrics {
        &self.data.metrics
    }

    /// Returns the range of text for the line.
    pub fn text_range(&self) -> Range<usize> {
        self.data.text_range.clone()
    }

    /// Returns the number of items in the line.
    pub fn len(&self) -> usize {
        self.data.item_range.len()
    }

    /// Returns true if the line is empty.
    pub fn is_empty(&self) -> bool {
        self.data.item_range.is_empty()
    }

    /// Returns the run at the specified index.
    pub fn get_item(&self, index: usize) -> Option<&LineItemData> {
        let index = self.data.item_range.start + index;
        if index >= self.data.item_range.end {
            return None;
        }
        let item = self.layout.line_items.get(index)?;
        Some(item)
    }

    /// Returns the run at the specified index.
    pub fn get_run(&self, index: usize) -> Option<Run<'a, B>> {
        let index = self.data.item_range.start + index;
        if index >= self.data.item_range.end {
            return None;
        }
        let item = self.layout.line_items.get(index)?;

        if item.kind == LayoutItemKind::TextRun {
            Some(Run {
                layout: self.layout,
                data: self.layout.runs.get(item.index)?,
                line_data: Some(item),
            })
        } else {
            None
        }
    }

    /// Returns an iterator over the runs for the line.
    // TODO: provide iterator over inline_boxes and items
    pub fn runs(&self) -> impl Iterator<Item = Run<'a, B>> + 'a + Clone {
        let copy = self.clone();
        let line_items = &copy.layout.line_items[self.data.item_range.clone()];
        line_items
            .iter()
            .filter(|item| item.kind == LayoutItemKind::TextRun)
            .map(move |line_data| Run {
                layout: copy.layout,
                data: &copy.layout.runs[line_data.index],
                line_data: Some(line_data),
            })
    }

    /// Returns an iterator over the glyph runs for the line.
    pub fn items(&self) -> impl Iterator<Item = LayoutItem2<'a, B>> + 'a + Clone {
        GlyphRunIter {
            line: self.clone(),
            item_index: 0,
            glyph_start: 0,
            offset: 0.,
        }
    }
}

/// Metrics information for a line.
#[derive(Copy, Clone, Default, Debug)]
pub struct LineMetrics {
    /// Typographic ascent.
    pub ascent: f32,
    /// Typographic descent.
    pub descent: f32,
    /// Typographic leading.
    pub leading: f32,
    /// Offset to the baseline.
    pub baseline: f32,
    /// Offset for alignment.
    pub offset: f32,
    /// Full advance of the line.
    pub advance: f32,
    /// Advance of trailing whitespace.
    pub trailing_whitespace: f32,
}

impl LineMetrics {
    /// Returns the size of the line (ascent + descent + leading).
    pub fn size(&self) -> f32 {
        self.ascent + self.descent + self.leading
    }
}

#[derive(Clone)]
pub enum LayoutItem2<'a, B: Brush> {
    GlyphRun(GlyphRun<'a, B>),
    InlineBox(PositionedInlineBox),
}

#[derive(Debug, Clone)]
pub struct PositionedInlineBox {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub id: u64,
}

/// Sequence of fully positioned glyphs with the same style.
#[derive(Clone)]
pub struct GlyphRun<'a, B: Brush> {
    run: Run<'a, B>,
    style: &'a Style<B>,
    glyph_start: usize,
    glyph_count: usize,
    offset: f32,
    baseline: f32,
    advance: f32,
}

impl<'a, B: Brush> GlyphRun<'a, B> {
    /// Returns the underlying run.
    pub fn run(&self) -> &Run<'a, B> {
        &self.run
    }

    /// Returns the associated style.
    pub fn style(&self) -> &Style<B> {
        self.style
    }

    /// Returns the offset to the baseline.
    pub fn baseline(&self) -> f32 {
        self.baseline
    }

    /// Returns the offset to the first glyph along the baseline.
    pub fn offset(&self) -> f32 {
        self.offset
    }

    /// Returns the total advance of the run.
    pub fn advance(&self) -> f32 {
        self.advance
    }

    /// Returns an iterator over the glyphs in the run.
    pub fn glyphs(&'a self) -> impl Iterator<Item = Glyph> + 'a + Clone {
        self.run
            .visual_clusters()
            .flat_map(|cluster| cluster.glyphs())
            .skip(self.glyph_start)
            .take(self.glyph_count)
    }

    /// Returns an iterator over the fully positioned glyphs in the run.
    pub fn positioned_glyphs(&'a self) -> impl Iterator<Item = Glyph> + 'a + Clone {
        let mut offset = self.offset;
        let baseline = self.baseline;
        self.run
            .visual_clusters()
            .flat_map(|cluster| cluster.glyphs())
            .skip(self.glyph_start)
            .take(self.glyph_count)
            .map(move |mut g| {
                g.x += offset;
                g.y += baseline;
                offset += g.advance;
                g
            })
    }
}

#[derive(Clone)]
struct GlyphRunIter<'a, B: Brush> {
    line: Line<'a, B>,
    item_index: usize,
    glyph_start: usize,
    offset: f32,
}

impl<'a, B: Brush> Iterator for GlyphRunIter<'a, B> {
    type Item = LayoutItem2<'a, B>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let item = self.line.get_item(self.item_index)?;
            match item.kind {
                LayoutItemKind::InlineBox => {
                    let inline_box = &self.line.layout.inline_boxes[item.index];

                    let x = self.offset + self.line.data.metrics.offset;

                    self.item_index += 1;
                    self.glyph_start = 0;
                    self.offset += item.advance;
                    return Some(LayoutItem2::InlineBox(PositionedInlineBox {
                        x,
                        y: self.line.data.metrics.baseline - inline_box.height,
                        width: inline_box.width,
                        height: inline_box.height,
                        id: inline_box.id,
                    }));
                }

                LayoutItemKind::TextRun => {
                    let run = self.line.get_run(self.item_index)?;
                    let mut iter = run
                        .visual_clusters()
                        .flat_map(|c| c.glyphs())
                        .skip(self.glyph_start);

                    if let Some(first) = iter.next() {
                        let mut advance = first.advance;
                        let style_index = first.style_index();
                        let mut glyph_count = 1;
                        for glyph in iter.take_while(|g| g.style_index() == style_index) {
                            glyph_count += 1;
                            advance += glyph.advance;
                        }
                        let style = run.layout.styles.get(style_index)?;
                        let glyph_start = self.glyph_start;
                        self.glyph_start += glyph_count;
                        let offset = self.offset;
                        self.offset += advance;
                        return Some(LayoutItem2::GlyphRun(GlyphRun {
                            run,
                            style,
                            glyph_start,
                            glyph_count,
                            offset: offset + self.line.data.metrics.offset,
                            baseline: self.line.data.metrics.baseline,
                            advance,
                        }));
                    }
                    self.item_index += 1;
                    self.glyph_start = 0;
                }
            }
        }
    }
}
