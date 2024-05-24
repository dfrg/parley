// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Greedy line breaking.

#[cfg(not(feature = "std"))]
use alloc::vec::Vec;

use crate::layout::*;
use crate::style::Brush;

use core::ops::Range;

#[derive(Default)]
struct LineLayout {
    lines: Vec<LineData>,
    line_items: Vec<LineItemData>,
}

impl LineLayout {
    fn swap<B: Brush>(&mut self, layout: &mut LayoutData<B>) {
        core::mem::swap(&mut self.lines, &mut layout.lines);
        core::mem::swap(&mut self.line_items, &mut layout.line_items);
    }
}

#[derive(Clone, Default)]
struct LineState {
    x: f32,
    runs: Range<usize>,
    clusters: Range<usize>,
    skip_mandatory_break: bool,
    num_spaces: usize,
}

#[derive(Clone, Default)]
struct PrevBoundaryState {
    i: usize,
    j: usize,
    state: LineState,
}

#[derive(Clone, Default)]
struct BreakerState {
    runs: usize,
    lines: usize,
    i: usize,
    j: usize,
    line: LineState,
    prev_boundary: Option<PrevBoundaryState>,
}

/// Line breaking support for a paragraph.
pub struct BreakLines<'a, B: Brush> {
    layout: &'a mut LayoutData<B>,
    lines: LineLayout,
    state: BreakerState,
    prev_state: Option<BreakerState>,
    done: bool,
}

impl<'a, B: Brush> BreakLines<'a, B> {
    pub(crate) fn new(layout: &'a mut LayoutData<B>) -> Self {
        unjustify(layout);
        layout.width = 0.;
        layout.height = 0.;
        let mut lines = LineLayout::default();
        lines.swap(layout);
        lines.lines.clear();
        lines.line_items.clear();
        Self {
            layout,
            lines,
            state: BreakerState::default(),
            prev_state: None,
            done: false,
        }
    }

    /// Computes the next line in the paragraph. Returns the advance and size
    /// (width and height for horizontal layouts) of the line.
    pub fn break_next(&mut self, max_advance: f32, alignment: Alignment) -> Option<(f32, f32)> {
        if self.done {
            return None;
        }
        self.prev_state = Some(self.state.clone());
        let run_count = self.layout.runs.len();
        while self.state.i < run_count {
            let run_data = &self.layout.runs[self.state.i];
            let run = Run::new(self.layout, run_data, None);
            let cluster_start = run_data.cluster_range.start;
            let cluster_end = run_data.cluster_range.end;
            while self.state.j < cluster_end {
                let cluster = run.get(self.state.j - cluster_start).unwrap();
                let is_ligature_continuation = cluster.is_ligature_continuation();
                let is_space = cluster.info().whitespace().is_space_or_nbsp();
                let boundary = cluster.info().boundary();
                match boundary {
                    Boundary::Mandatory => {
                        if !self.state.line.skip_mandatory_break {
                            self.state.prev_boundary = None;
                            self.state.line.clusters.end = self.state.j;
                            self.state.line.runs.end = self.state.i + 1;
                            self.state.line.skip_mandatory_break = true;
                            if commit_line(
                                self.layout,
                                &mut self.lines,
                                &mut self.state.line,
                                max_advance,
                                alignment,
                                BreakReason::Explicit,
                                false,
                            ) {
                                self.state.runs = self.lines.line_items.len();
                                self.state.lines = self.lines.lines.len();
                                self.state.line.x = 0.;
                                let line = self.lines.lines.last().unwrap();
                                return Some((line.metrics.advance, line.size()));
                            }
                        }
                    }
                    Boundary::Line => {
                        if !is_ligature_continuation {
                            self.state.prev_boundary = Some(PrevBoundaryState {
                                i: self.state.i,
                                j: self.state.j,
                                state: self.state.line.clone(),
                            });
                        }
                    }
                    _ => {}
                }
                self.state.line.skip_mandatory_break = false;
                let mut advance = cluster.advance();
                if cluster.is_ligature_start() {
                    while let Some(cluster) = run.get(self.state.j + 1) {
                        if !cluster.is_ligature_continuation() {
                            break;
                        } else {
                            advance += cluster.advance();
                            self.state.j += 1;
                        }
                    }
                }
                let next_x = self.state.line.x + advance;
                if next_x > max_advance {
                    if is_space {
                        // Hang overflowing whitespace
                        self.state.line.runs.end = self.state.i + 1;
                        self.state.line.clusters.end = self.state.j + 1;
                        self.state.line.x = next_x;
                        if commit_line(
                            self.layout,
                            &mut self.lines,
                            &mut self.state.line,
                            max_advance,
                            alignment,
                            BreakReason::Regular,
                            false,
                        ) {
                            self.state.runs = self.lines.line_items.len();
                            self.state.lines = self.lines.lines.len();
                            self.state.line.x = 0.;
                            let line = self.lines.lines.last().unwrap();
                            self.state.prev_boundary = None;
                            self.state.j += 1;
                            return Some((line.metrics.advance, line.size()));
                        }
                    } else if let Some(prev) = self.state.prev_boundary.take() {
                        if prev.state.x == 0. {
                            // This will cycle if we try to rewrap. Accept the overflowing fragment.
                            self.state.line.runs.end = self.state.i + 1;
                            self.state.line.clusters.end = self.state.j + 1;
                            self.state.line.x = next_x;
                            self.state.j += 1;
                            if commit_line(
                                self.layout,
                                &mut self.lines,
                                &mut self.state.line,
                                max_advance,
                                alignment,
                                BreakReason::Emergency,
                                false,
                            ) {
                                self.state.runs = self.lines.line_items.len();
                                self.state.lines = self.lines.lines.len();
                                self.state.line.x = 0.;
                                let line = self.lines.lines.last().unwrap();
                                self.state.prev_boundary = None;
                                self.state.j += 1;
                                return Some((line.metrics.advance, line.size()));
                            }
                        } else {
                            self.state.line = prev.state;
                            if commit_line(
                                self.layout,
                                &mut self.lines,
                                &mut self.state.line,
                                max_advance,
                                alignment,
                                BreakReason::Regular,
                                false,
                            ) {
                                self.state.runs = self.lines.line_items.len();
                                self.state.lines = self.lines.lines.len();
                                self.state.line.x = 0.;
                                let line = self.lines.lines.last().unwrap();
                                self.state.i = prev.i;
                                self.state.j = prev.j;
                                return Some((line.metrics.advance, line.size()));
                            }
                        }
                    } else {
                        if self.state.line.x == 0. {
                            // If we're at the start of the line, this particular
                            // cluster will never fit, so consume it and accept
                            // the overflow.
                            self.state.line.runs.end = self.state.i + 1;
                            self.state.line.clusters.end = self.state.j + 1;
                            self.state.line.x = next_x;
                            self.state.j += 1;
                        }
                        if commit_line(
                            self.layout,
                            &mut self.lines,
                            &mut self.state.line,
                            max_advance,
                            alignment,
                            BreakReason::Emergency,
                            false,
                        ) {
                            self.state.runs = self.lines.line_items.len();
                            self.state.lines = self.lines.lines.len();
                            self.state.line.x = 0.;
                            let line = self.lines.lines.last().unwrap();
                            self.state.prev_boundary = None;
                            self.state.j += 1;
                            return Some((line.metrics.advance, line.size()));
                        }
                    }
                } else {
                    // Commit the cluster to the line.
                    self.state.line.runs.end = self.state.i + 1;
                    self.state.line.clusters.end = self.state.j + 1;
                    self.state.line.x = next_x;
                    self.state.j += 1;
                    if is_space {
                        self.state.line.num_spaces += 1;
                    }
                }
            }
            self.state.i += 1;
        }
        if commit_line(
            self.layout,
            &mut self.lines,
            &mut self.state.line,
            max_advance,
            alignment,
            BreakReason::None,
            true,
        ) {
            self.state.runs = self.lines.line_items.len();
            self.state.lines = self.lines.lines.len();
            self.state.line.x = 0.;
            let line = self.lines.lines.last().unwrap();
            self.done = true;
            return Some((line.metrics.advance, line.size()));
        }
        None
    }

    /// Reverts the last computed line, returning to the previous state.
    pub fn revert(&mut self) -> bool {
        if let Some(state) = self.prev_state.take() {
            self.state = state;
            self.lines.lines.truncate(self.state.lines);
            self.lines.line_items.truncate(self.state.runs);
            self.done = false;
            true
        } else {
            false
        }
    }

    /// Breaks all remaining lines with the specified maximum advance. This
    /// consumes the line breaker.
    pub fn break_remaining(mut self, max_advance: f32, alignment: Alignment) {
        while self.break_next(max_advance, alignment).is_some() {}
        self.finish();
    }

    /// Consumes the line breaker and finalizes all line computations.
    pub fn finish(mut self) {
        for run in &mut self.lines.line_items {
            run.is_whitespace = true;
            if run.bidi_level & 1 != 0 {
                // RTL runs check for "trailing" whitespace at the front.
                for cluster in self.layout.clusters[run.cluster_range.clone()].iter() {
                    if cluster.info.is_whitespace() {
                        run.has_trailing_whitespace = true;
                    } else {
                        run.is_whitespace = false;
                        break;
                    }
                }
            } else {
                for cluster in self.layout.clusters[run.cluster_range.clone()].iter().rev() {
                    if cluster.info.is_whitespace() {
                        run.has_trailing_whitespace = true;
                    } else {
                        run.is_whitespace = false;
                        break;
                    }
                }
            }
        }
        let mut y = 0.;
        for line in &mut self.lines.lines {
            let run_base = line.run_range.start;
            let run_count = line.run_range.end - run_base;

            // Reset mutable state for line
            line.metrics.ascent = 0.;
            line.metrics.descent = 0.;
            line.metrics.leading = 0.;
            line.metrics.offset = 0.;
            let mut have_metrics = false;
            let mut needs_reorder = false;
            line.text_range.start = usize::MAX;


            // Compute metrics for the line, but ignore trailing whitespace.
            for line_run in self.lines.line_items[line.run_range.clone()]
                .iter_mut()
                .rev()
            {
                line.text_range.end = line.text_range.end.max(line_run.text_range.end);
                line.text_range.start = line.text_range.start.min(line_run.text_range.start);
                if line_run.bidi_level != 0 {
                    needs_reorder = true;
                }
                if !have_metrics && line_run.is_whitespace {
                    continue;
                }
                line_run.advance = self.layout.clusters[line_run.cluster_range.clone()]
                    .iter()
                    .map(|c| c.advance)
                    .sum();
                let line_height = line_run.compute_line_height(self.layout);
                let run = &self.layout.runs[line_run.index];
                line.metrics.ascent = line.metrics.ascent.max(run.metrics.ascent * line_height);
                line.metrics.descent = line.metrics.descent.max(run.metrics.descent * line_height);
                line.metrics.leading = line.metrics.leading.max(run.metrics.leading * line_height);
                have_metrics = true;
            }
            if needs_reorder && run_count > 1 {
                reorder_line_items(&mut self.lines.line_items[line.run_range.clone()]);
            }

            // Compute size of line's trailing whitespace
            let trailing_whitespace = if !line.run_range.is_empty() {
                let last_run = &self.lines.line_items[line.run_range.end - 1];
                if !last_run.cluster_range.is_empty() {
                    let cluster = &self.layout.clusters[last_run.cluster_range.end - 1];
                    if cluster.info.whitespace().is_space_or_nbsp() {
                        cluster.advance
                    } else {
                        0.
                    }
                } else {
                    0.
                }
            } else {
                0.
            };
            line.metrics.trailing_whitespace = trailing_whitespace;

            // Justify line items
            let has_finite_width = line.max_advance.is_finite() && line.max_advance < f32::MAX;
            if line.alignment != Alignment::Start && has_finite_width {
                let extra = line.max_advance - line.metrics.advance + trailing_whitespace;
                if extra > 0. {
                    let offset = if line.alignment == Alignment::Middle {
                        extra * 0.5
                    } else {
                        extra
                    };
                    if line.alignment == Alignment::Justified {
                        if line.break_reason != BreakReason::None && line.num_spaces != 0 {
                            let adjustment = extra / line.num_spaces as f32;
                            let mut applied = 0;
                            for line_run in &self.lines.line_items[line.run_range.clone()] {
                                if line_run.bidi_level & 1 != 0 {
                                    for cluster in self.layout.clusters
                                        [line_run.cluster_range.clone()]
                                    .iter_mut()
                                    .rev()
                                    {
                                        if applied == line.num_spaces {
                                            break;
                                        }
                                        if cluster.info.whitespace().is_space_or_nbsp() {
                                            cluster.advance += adjustment;
                                            applied += 1;
                                        }
                                    }
                                } else {
                                    for cluster in self.layout.clusters
                                        [line_run.cluster_range.clone()]
                                    .iter_mut()
                                    {
                                        if applied == line.num_spaces {
                                            break;
                                        }
                                        if cluster.info.whitespace().is_space_or_nbsp() {
                                            cluster.advance += adjustment;
                                            applied += 1;
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        line.metrics.offset = offset;
                    }
                }
            }

            if !have_metrics {
                // Line consisting entirely of whitespace?
                if !line.run_range.is_empty() {
                    let line_run = &self.lines.line_items[line.run_range.start];
                    let run = &self.layout.runs[line_run.index];
                    line.metrics.ascent = run.metrics.ascent;
                    line.metrics.descent = run.metrics.descent;
                    line.metrics.leading = run.metrics.leading;
                }
            }

            // Round block/vertical axis metrics
            line.metrics.ascent = line.metrics.ascent.round();
            line.metrics.descent = line.metrics.descent.round();
            line.metrics.leading = (line.metrics.leading * 0.5).round() * 2.;

            // Compute
            let above = (line.metrics.ascent + line.metrics.leading * 0.5).round();
            let below = (line.metrics.descent + line.metrics.leading * 0.5).round();
            line.metrics.baseline = y + above;
            y = line.metrics.baseline + below;
        }
    }
}

impl<'a, B: Brush> Drop for BreakLines<'a, B> {
    fn drop(&mut self) {
        let mut width = 0f32;
        let mut full_width = 0f32;
        let mut height = 0f32;
        for line in &self.lines.lines {
            width = width.max(line.metrics.advance - line.metrics.trailing_whitespace);
            full_width = full_width.max(line.metrics.advance);
            height += line.metrics.size();
        }
        self.layout.width = width;
        self.layout.full_width = full_width;
        self.layout.height = height;
        self.lines.swap(self.layout);
    }
}

/// Removes previous justification applied to clusters.
fn unjustify<B: Brush>(layout: &mut LayoutData<B>) {
    for line in &layout.lines {
        if line.alignment == Alignment::Justified
            && line.max_advance.is_finite()
            && line.max_advance < f32::MAX
        {
            let extra = line.max_advance - line.metrics.advance + line.metrics.trailing_whitespace;
            if line.break_reason != BreakReason::None && line.num_spaces != 0 {
                let adjustment = extra / line.num_spaces as f32;
                let mut applied = 0;
                for line_run in &layout.line_items[line.run_range.clone()] {
                    if line_run.bidi_level & 1 != 0 {
                        for cluster in layout.clusters[line_run.cluster_range.clone()]
                            .iter_mut()
                            .rev()
                        {
                            if applied == line.num_spaces {
                                break;
                            }
                            if cluster.info.whitespace().is_space_or_nbsp() {
                                cluster.advance -= adjustment;
                                applied += 1;
                            }
                        }
                    } else {
                        for cluster in layout.clusters[line_run.cluster_range.clone()].iter_mut() {
                            if applied == line.num_spaces {
                                break;
                            }
                            if cluster.info.whitespace().is_space_or_nbsp() {
                                cluster.advance -= adjustment;
                                applied += 1;
                            }
                        }
                    }
                }
            }
        }
    }
}

fn commit_line<B: Brush>(
    layout: &LayoutData<B>,
    lines: &mut LineLayout,
    state: &mut LineState,
    max_advance: f32,
    alignment: Alignment,
    break_reason: BreakReason,
    is_last: bool,
) -> bool {
    let is_empty = layout.text_len == 0;
    state.clusters.end = state.clusters.end.min(layout.clusters.len());
    if state.runs.end == 0 && is_last {
        state.runs.end = 1;
    }
    let last_run = state.runs.len() - 1;
    let runs_start = lines.line_items.len();
    for (i, run_data) in layout.runs[state.runs.clone()].iter().enumerate() {
        let mut cluster_range = run_data.cluster_range.clone();
        if i == 0 {
            cluster_range.start = state.clusters.start;
        }
        if i == last_run {
            cluster_range.end = state.clusters.end;
        }

        // Skip empty/invalid clusters
        if cluster_range.start > cluster_range.end
            || (!is_empty && cluster_range.start == cluster_range.end)
        {
            continue;
        }

        // Push run to line
        let run = Run::new(layout, run_data, None);
        let text_range = if run_data.cluster_range.is_empty() {
            0..0
        } else {
            let first_cluster = run
                .get(cluster_range.start - run_data.cluster_range.start)
                .unwrap();
            let last_cluster = run
                .get((cluster_range.end - run_data.cluster_range.start).saturating_sub(1))
                .unwrap();
            first_cluster.text_range().start..last_cluster.text_range().end
        };

        // TODO: check that this is correct with boxes
        let index = state.runs.start + i;
        let line_run = LineItemData {
            kind: LayoutItemKind::TextRun,
            index,
            bidi_level: run_data.bidi_level,
            is_whitespace: false,
            has_trailing_whitespace: false,
            cluster_range,
            text_range,
            advance: 0.,
        };
        lines.line_items.push(line_run);
    }
    let runs_end = lines.line_items.len();

    // If no runs have been added to the line then we cannot have become ready to
    // commit the line (as we have not changed it at all)
    //
    // TODO: work out exactly when this happens
    if runs_start == runs_end {
        return false;
    }

    let mut num_spaces = state.num_spaces;
    if break_reason == BreakReason::Regular {
        num_spaces = num_spaces.saturating_sub(1);
    }
    let mut line = LineData {
        run_range: runs_start..runs_end,
        max_advance,
        alignment,
        break_reason,
        num_spaces,
        ..Default::default()
    };
    line.metrics.advance = state.x;
    lines.lines.push(line);
    state.clusters.start = state.clusters.end;
    state.clusters.end += 1;
    state.runs.start = state.runs.end - 1;
    state.num_spaces = 0;
    true
}

/// Reorder items within line according to the bidi levels of the items
fn reorder_line_items(runs: &mut [LineItemData]) {
    let run_count = runs.len();

    // Find the max level and the min *odd* level
    let mut max_level = 0;
    let mut lowest_odd_level = 255;
    for run in runs.iter() {
        let level = run.bidi_level;
        let is_odd = level & 1 != 0;

        // Update max level
        if level > max_level {
            max_level = level;
        }

        // Update min odd level
        if is_odd && level < lowest_odd_level {
            lowest_odd_level = level;
        }
    }

    // Interate over bidi levels
    for level in (lowest_odd_level..=max_level).rev() {
        // Iterate over text runs
        let mut i = 0;
        while i < run_count {
            if runs[i].bidi_level >= level {
                let mut end = i + 1;
                while end < run_count && runs[end].bidi_level >= level {
                    end += 1;
                }

                let mut j = i;
                let mut k = end - 1;
                while j < k {
                    runs.swap(j, k);
                    j += 1;
                    k -= 1;
                }

                i = end;
            }
            i += 1;
        }
    }
}
