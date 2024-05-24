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
    item_idx: usize,
    run_idx: usize,
    cluster_idx: usize,
    state: LineState,
}

#[derive(Clone, Default)]
struct BreakerState {
    /// The number of items that have been processed (used to revert state)
    items: usize,
    /// The number of items that have been processed (used to revert state)
    lines: usize,

    /// Iteration state: the current item (within the layout)
    item_idx: usize,
    /// Iteration state: the current run (within the layout)
    run_idx: usize,
    /// Iteration state: the current cluster (within the layout)
    cluster_idx: usize,

    line: LineState,
    prev_boundary: Option<PrevBoundaryState>,
}

impl BreakerState {
    /// Add the cluster(s) currently being evaluated to the current line
    fn append_cluster_to_line(&mut self, next_x: f32) {
        self.line.runs.end = self.run_idx + 1;
        self.line.clusters.end = self.cluster_idx + 1;
        self.line.x = next_x;
        // Would like to add:
        // self.cluster_idx += 1;
    }

    /// Store the current iteration state so that we can revert to it if we later want to take
    /// the line breaking opportunity at this point.
    fn mark_line_break_opportunity(&mut self) {
        self.prev_boundary = Some(PrevBoundaryState {
            item_idx: self.item_idx,
            run_idx: self.run_idx,
            cluster_idx: self.cluster_idx,
            state: self.line.clone(),
        });
    }
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

    /// Reset state when a line has been comitted
    fn start_new_line(&mut self) -> Option<(f32, f32)> {
        self.state.items = self.lines.line_items.len();
        self.state.lines = self.lines.lines.len();
        self.state.line.x = 0.;
        // Would like to add:
        // self.state.prev_boundary = None;
        self.last_line_data()
    }

    fn last_line_data(&self) -> Option<(f32, f32)> {
        let line = self.lines.lines.last().unwrap();
        Some((line.metrics.advance, line.size()))
    }

    /// Computes the next line in the paragraph. Returns the advance and size
    /// (width and height for horizontal layouts) of the line.
    pub fn break_next(&mut self, max_advance: f32, alignment: Alignment) -> Option<(f32, f32)> {
        // Maintain iterator state
        if self.done {
            return None;
        }
        self.prev_state = Some(self.state.clone());

        // This macro simply calls the `commit_line` with the provided arguments and some parts of self.
        // It exists solely to cut down on the boilerplate for accessing the self variables while
        // keeping the borrow checker happy
        macro_rules! try_commit {
            ($break_reason:expr) => {
                commit_line(
                    self.layout,
                    &mut self.lines,
                    &mut self.state.line,
                    max_advance,
                    alignment,
                    $break_reason,
                )
            };
        }

        // Iterate over remaining runs in the Layout
        let run_count = self.layout.runs.len();
        while self.state.run_idx < run_count {
            let run_data = &self.layout.runs[self.state.run_idx];
            let run = Run::new(self.layout, run_data, None);
            let cluster_start = run_data.cluster_range.start;
            let cluster_end = run_data.cluster_range.end;

            // Iterate over remaining clusters in the Run
            while self.state.cluster_idx < cluster_end {
                let cluster = run.get(self.state.cluster_idx - cluster_start).unwrap();

                // Retrieve metadata about the cluster
                let is_ligature_continuation = cluster.is_ligature_continuation();
                let is_space = cluster.info().whitespace().is_space_or_nbsp();
                let boundary = cluster.info().boundary();

                // Handle boundary clusters
                match boundary {
                    // A hard line break (e.g. CRLF or similar)
                    Boundary::Mandatory => {
                        if !self.state.line.skip_mandatory_break {
                            self.state.prev_boundary = None;
                            self.state.line.runs.end = self.state.run_idx + 1;
                            self.state.line.clusters.end = self.state.cluster_idx;

                            // We skip a mandatory break immediately after another mandatory break
                            self.state.line.skip_mandatory_break = true;

                            if try_commit!(BreakReason::Explicit) {
                                return self.start_new_line();
                            }
                        }
                    }
                    // A line-breaking opportunity. We save our state at this point so that we can later go back and
                    // "take" the line-breaking opportunity.
                    Boundary::Line => {
                        // We do not currently handle breaking within a ligature, so we ignore boundaries in such a position.
                        if !is_ligature_continuation {
                            self.state.mark_line_break_opportunity();
                        }
                    }
                    // Not a line boundary
                    _ => {}
                }

                // We only skip a mandatory break immediately after another mandatory break so if we
                // have got this far then we should disable mandatory break skipping
                self.state.line.skip_mandatory_break = false;

                // If current cluster is the start of a ligature, then advance state to include
                // the remaining clusters that make up the ligature
                let mut advance = cluster.advance();
                if cluster.is_ligature_start() {
                    while let Some(cluster) = run.get(self.state.cluster_idx + 1) {
                        if !cluster.is_ligature_continuation() {
                            break;
                        } else {
                            advance += cluster.advance();
                            self.state.cluster_idx += 1;
                        }
                    }
                }

                // Compute the x position of the content being currently processed
                let next_x = self.state.line.x + advance;

                // If that x position does NOT exceed max_advance then we simply add the cluster(s) to the current line
                if next_x <= max_advance {
                    self.state.line.runs.end = self.state.run_idx + 1;
                    self.state.line.clusters.end = self.state.cluster_idx + 1;
                    self.state.line.x = next_x;
                    self.state.cluster_idx += 1;
                    if is_space {
                        self.state.line.num_spaces += 1;
                    }
                }
                // Else we line break:
                else {
                    // Handle case where cluster is space character. Hang overflowing whitespace.
                    if is_space {
                        self.state.append_cluster_to_line(next_x);
                        if try_commit!(BreakReason::Regular) {
                            // TODO: can this be hoisted out of the conditional?
                            self.state.cluster_idx += 1;
                            self.state.prev_boundary = None;
                            return self.start_new_line();
                        }
                    }
                    // Handle the (common) case where we have previously encountered a line-breaking opportunity in the current line
                    else if let Some(prev) = self.state.prev_boundary.take() {
                        // If the previous line-breaking opportunity was at x=0 (i.e. the very start of a line) then we shouldn't
                        // insert another line break. We should accept the overflowing fragment on the current line.
                        if prev.state.x == 0. {
                            self.state.append_cluster_to_line(next_x);
                            self.state.cluster_idx += 1;

                            if try_commit!(BreakReason::Emergency) {
                                self.state.cluster_idx += 1;
                                self.state.prev_boundary = None;
                                return self.start_new_line();
                            }
                        }
                        // Otherwise we "take" the line-breaking opportunity by starting a new line and resetting our
                        // item/run/cluster iteration state back to how it was when the line-breaking opportunity was encountered
                        else {

                            // Q: Why do we revert the line state here, but only revert the indexes if the commit suceeds?
                            self.state.line = prev.state;
                            if try_commit!(BreakReason::Regular) {
                                // Revert boundary state to prev state
                                self.state.item_idx = prev.item_idx;
                                self.state.run_idx = prev.run_idx;
                                self.state.cluster_idx = prev.cluster_idx;

                                // This line wasn't previously here, but was implied by the self.state.prev_boundary.take() above
                                // Adding for consistency between branches.
                                self.state.prev_boundary = None;

                                return self.start_new_line();
                            }
                        }
                    }
                    // Otherwise perform an emergency line break
                    else {
                        // If we're at the start of the line, this particular cluster will never fit,
                        // so consume it and accept the overflow.
                        if self.state.line.x == 0. {
                            self.state.append_cluster_to_line(next_x);
                            self.state.cluster_idx += 1;
                        }
                        if try_commit!(BreakReason::Emergency) {
                            self.state.cluster_idx += 1;
                            self.state.prev_boundary = None;
                            return self.start_new_line();
                        }
                    }
                }
            }
            self.state.run_idx += 1;
        }

        if self.state.line.runs.end == 0 {
            self.state.line.runs.end = 1;
        }
        if try_commit!(BreakReason::None) {
            self.done = true;
            return self.start_new_line();
        }

        None
    }

    /// Reverts the last computed line, returning to the previous state.
    pub fn revert(&mut self) -> bool {
        if let Some(state) = self.prev_state.take() {
            self.state = state;
            self.lines.lines.truncate(self.state.lines);
            self.lines.line_items.truncate(self.state.items);
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

            // Reorder the items within the line (if required). Reordering is required if the line contains
            // a mix of bidi levels (a mix of LTR and RTL text)
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

            // Apply alignment to line items
            let has_finite_width = line.max_advance.is_finite() && line.max_advance < f32::MAX;
            if has_finite_width {
                // Compute free space. Alignment only applies if free_space > 0
                let free_space = line.max_advance - line.metrics.advance + trailing_whitespace;
                if free_space > 0. {
                    match line.alignment {
                        Alignment::Start => {
                            // Do nothing
                        }
                        Alignment::End => {
                            line.metrics.offset = free_space;
                        }
                        Alignment::Middle => {
                            line.metrics.offset = free_space * 0.5;
                        }
                        Alignment::Justified => {
                            if line.break_reason != BreakReason::None && line.num_spaces != 0 {
                                let adjustment = free_space / line.num_spaces as f32;
                                let mut applied = 0;
                                for line_run in &self.lines.line_items[line.run_range.clone()] {
                                    let clusters =
                                        &mut self.layout.clusters[line_run.cluster_range.clone()];

                                    // Iterate over clusters in the run
                                    //   - Iterate forwards for even bidi levels (which represent RTL runs)
                                    //   - Iterate backwards for odd bidi levels (which represent RTL runs)
                                    let bidi_level_is_odd = line_run.bidi_level & 1 != 0;
                                    if bidi_level_is_odd {
                                        for cluster in clusters.iter_mut().rev() {
                                            if applied == line.num_spaces {
                                                break;
                                            }
                                            if cluster.info.whitespace().is_space_or_nbsp() {
                                                cluster.advance += adjustment;
                                                applied += 1;
                                            }
                                        }
                                    } else {
                                        for cluster in clusters.iter_mut() {
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
                        }
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

// fn cluster_range_is_valid(
//     mut cluster_range: Range<usize>,
//     state_cluster_range: Range<usize>,
//     is_first: bool,
//     is_last: bool,
//     is_empty: bool,
// ) -> bool {
//     // Compute cluster range
//     if is_first {
//         cluster_range.start = state_cluster_range.start;
//     }
//     if is_last {
//         cluster_range.end = state_cluster_range.end;
//     }

//     // Return true if cluster is valid. Else false.
//     cluster_range.start < cluster_range.end
//         || (cluster_range.start == cluster_range.end && is_empty)
// }

// fn should_commit_line<B: Brush>(
//     layout: &LayoutData<B>,
//     state: &mut LineState,
//     is_last: bool,
// ) -> bool {
//     // Compute end cluster
//     state.clusters.end = state.clusters.end.min(layout.clusters.len());
//     if state.runs.end == 0 && is_last {
//         state.runs.end = 1;
//     }

//     let last_run = state.runs.len() - 1;
//     let is_empty = layout.text_len == 0;

//     // Iterate over runs. Checking if any have a valid cluster range.
//     let runs = &layout.runs[state.runs.clone()];
//     runs.iter().enumerate().any(|(i, run_data)| {
//         cluster_range_is_valid(
//             run_data.cluster_range.clone(),
//             state.clusters.clone(),
//             i == 0,
//             i == last_run,
//             is_empty,
//         )
//     })
// }

fn commit_line<B: Brush>(
    layout: &LayoutData<B>,
    lines: &mut LineLayout,
    state: &mut LineState,
    max_advance: f32,
    alignment: Alignment,
    break_reason: BreakReason,
) -> bool {
    let is_empty = layout.text_len == 0;

    state.clusters.end = state.clusters.end.min(layout.clusters.len());

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
