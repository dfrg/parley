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
    items: Range<usize>,
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
        self.line.items.end = self.item_idx + 1;
        self.line.clusters.end = self.cluster_idx + 1;
        self.line.x = next_x;
        // Would like to add:
        // self.cluster_idx += 1;
    }

    /// Add inline box to line
    fn append_inline_box_to_line(&mut self, next_x: f32) {
        // self.item_idx += 1;
        self.line.items.end += 1;
        self.line.x = next_x;
        // Would like to add:
        // self.item_idx += 1;
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
        self.state.prev_boundary = None; // Added by Nico
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
        macro_rules! try_commit_line {
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

        // dbg!(&self.layout.items);

        // println!("\nBREAK NEXT");
        // dbg!(&self.state.line.items);

        // Iterate over remaining runs in the Layout
        let item_count = self.layout.items.len();
        while self.state.item_idx < item_count {
            let item = &self.layout.items[self.state.item_idx];

            // println!(
            //     "\nitem = {} {:?}. x: {}",
            //     self.state.item_idx, item.kind, self.state.line.x
            // );
            // dbg!(&self.state.line.items);

            match item.kind {
                LayoutItemKind::InlineBox => {
                    let inline_box = &self.layout.inline_boxes[item.index];

                    // Compute the x position of the content being currently processed
                    let next_x = self.state.line.x + inline_box.width;

                    // println!("BOX next_x: {}", next_x);

                    // If the box fits on the current line (or we are at the start of the current line)
                    // then simply move on to the next item
                    if next_x <= max_advance {
                        // println!("BOX FITS");

                        self.state.item_idx += 1;
                        self.state.append_inline_box_to_line(next_x);

                        // We can always line break after an inline box
                        self.state.mark_line_break_opportunity();
                    } else {
                        // If we're at the start of the line, this box will never fit, so consume it and accept the overflow.
                        if self.state.line.x == 0.0 {
                            // println!("BOX EMERGENCY BREAK");
                            self.state.append_inline_box_to_line(next_x);
                            if try_commit_line!(BreakReason::Regular) {
                                self.state.item_idx += 1;
                                return self.start_new_line();
                            }
                        } else {
                            // println!("BOX BREAK");
                            if try_commit_line!(BreakReason::Regular) {
                                return self.start_new_line();
                            }
                        };
                    }
                }
                LayoutItemKind::TextRun => {
                    let run_idx = item.index;
                    let run_data = &self.layout.runs[run_idx];

                    let run = Run::new(self.layout, run_data, None);
                    let cluster_start = run_data.cluster_range.start;
                    let cluster_end = run_data.cluster_range.end;

                    // println!("TextRun ({:?})", &run_data.text_range);

                    // Iterate over remaining clusters in the Run
                    while self.state.cluster_idx < cluster_end {
                        let cluster = run.get(self.state.cluster_idx - cluster_start).unwrap();

                        // Retrieve metadata about the cluster
                        let is_ligature_continuation = cluster.is_ligature_continuation();
                        let is_space = cluster.info().whitespace().is_space_or_nbsp();
                        let boundary = cluster.info().boundary();

                        let mut break_opportunity = false;

                        // Handle boundary clusters
                        match boundary {
                            // A hard line break (e.g. CRLF or similar)
                            Boundary::Mandatory => {
                                // println!("TextRun (Mandatory Break)");

                                if !self.state.line.skip_mandatory_break {
                                    self.state.prev_boundary = None;
                                    self.state.line.items.end = self.state.item_idx + 1;
                                    self.state.line.clusters.end = self.state.cluster_idx;

                                    // We skip a mandatory break immediately after another mandatory break
                                    self.state.line.skip_mandatory_break = true;

                                    if try_commit_line!(BreakReason::Explicit) {
                                        return self.start_new_line();
                                    }
                                }
                            }
                            // A line-breaking opportunity. We save our state at this point so that we can later go back and
                            // "take" the line-breaking opportunity.
                            Boundary::Line => {
                                // We do not currently handle breaking within a ligature, so we ignore boundaries in such a position.
                                //
                                // We also don't record boundaries when the advance is 0. As we do not want overflowing content to cause extra consecutive
                                // line breaks. We should accept the overflowing fragment in that scenario.
                                if !is_ligature_continuation && self.state.line.x != 0.0 {
                                    self.state.mark_line_break_opportunity();
                                    break_opportunity = true;
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

                        // println!("Cluster {} next_x: {}", self.state.cluster_idx, next_x);

                        // if break_opportunity {
                        //     println!("===");
                        // }

                        // If that x position does NOT exceed max_advance then we simply add the cluster(s) to the current line
                        if next_x <= max_advance {
                            self.state.append_cluster_to_line(next_x);
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
                                if try_commit_line!(BreakReason::Regular) {
                                    // TODO: can this be hoisted out of the conditional?
                                    self.state.cluster_idx += 1;
                                    return self.start_new_line();
                                }
                            }
                            // Handle the (common) case where we have previously encountered a line-breaking opportunity in the current line
                            //
                            // We "take" the line-breaking opportunity by starting a new line and resetting our
                            // item/run/cluster iteration state back to how it was when the line-breaking opportunity was encountered
                            else if let Some(prev) = self.state.prev_boundary.take() {
                                // println!("REVERT");
                                debug_assert!(prev.state.x != 0.0);

                                // Q: Why do we revert the line state here, but only revert the indexes if the commit suceeds?
                                self.state.line = prev.state;
                                if try_commit_line!(BreakReason::Regular) {
                                    // Revert boundary state to prev state
                                    self.state.item_idx = prev.item_idx;
                                    self.state.run_idx = prev.run_idx;
                                    self.state.cluster_idx = prev.cluster_idx;

                                    return self.start_new_line();
                                }
                            }
                            // Otherwise perform an emergency line break
                            // TODO: make emergency line breaks controllable via a setting
                            else {
                                // If we're at the start of the line, this particular cluster will never fit,
                                // so consume it and accept the overflow.
                                if self.state.line.x == 0. {
                                    self.state.append_cluster_to_line(next_x);
                                    self.state.cluster_idx += 1;
                                    if is_space {
                                        self.state.line.num_spaces += 1;
                                    }

                                    if try_commit_line!(BreakReason::Emergency) {
                                        self.state.cluster_idx += 1;
                                        return self.start_new_line();
                                    }
                                }
                                else {
                                    const BREAK_WORDS_IN_EMERGENCY : bool = false;

                                    if BREAK_WORDS_IN_EMERGENCY {
                                        if try_commit_line!(BreakReason::Emergency) {
                                            self.state.cluster_idx += 1;
                                            return self.start_new_line();
                                        }
                                    } else {
                                        self.state.append_cluster_to_line(next_x);
                                        self.state.cluster_idx += 1;
                                        if is_space {
                                            self.state.line.num_spaces += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    self.state.run_idx += 1;
                    self.state.item_idx += 1;
                }
            }
        }

        if self.state.line.items.end == 0 {
            self.state.line.items.end = 1;
        }
        if try_commit_line!(BreakReason::None) {
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
        // println!("\nDEBUG ITEMS");
        // for item in &self.layout.items {
        //     match item.kind {
        //         LayoutItemKind::InlineBox => println!("{:?}", item.kind),
        //         LayoutItemKind::TextRun => {
        //             let run_data = &self.layout.runs[item.index];
        //             println!("{:?} ({:?})", item.kind, &run_data.text_range);
        //         }
        //     }
        // }

        // println!("\nBREAK ALL");

        while self.break_next(max_advance, alignment).is_some() {}
        self.finish();
    }

    /// Consumes the line breaker and finalizes all line computations.
    pub fn finish(mut self) {
        // For each run (item which is a text run):
        //   - Determine if it consists entirely of whitespace (is_whitespace property)
        //   - Determine if it has trailing whitespace (has_trailing_whitespace property)
        for item in &mut self.lines.line_items {
            // Skip items which are not text runs
            if item.kind != LayoutItemKind::TextRun {
                continue;
            }

            let run = item;
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
            // Reset metrics for line
            line.metrics.ascent = 0.;
            line.metrics.descent = 0.;
            line.metrics.leading = 0.;
            line.metrics.offset = 0.;
            line.text_range.start = usize::MAX;

            // Compute metrics for the line, but ignore trailing whitespace.
            let mut have_metrics = false;
            let mut needs_reorder = false;
            for line_item in self.lines.line_items[line.item_range.clone()]
                .iter_mut()
                .rev()
            {
                match line_item.kind {
                    LayoutItemKind::InlineBox => {
                        let item = &self.layout.inline_boxes[line_item.index];

                        // Advance is already computed in "commit line" for items

                        // Default vertical alignment is to align the bottom of boxes with the text baseline.
                        // This is equivalent to the entire height of the box being "ascent"
                        line.metrics.ascent = line.metrics.ascent.max(item.height);

                        // Mark us as having seen non-whitespace content on this line
                        have_metrics = true;
                    }
                    LayoutItemKind::TextRun => {
                        // Compute the text range for the line
                        // Q: Can we not simplify this computation by assuming that items are in order?
                        line.text_range.end = line.text_range.end.max(line_item.text_range.end);
                        line.text_range.start =
                            line.text_range.start.min(line_item.text_range.start);

                        // Mark line as needing bidi re-ordering if it contains any runs with non-zero bidi level
                        // (zero is the default level, so this is equivalent to marking lines that have multiple levels)
                        if line_item.bidi_level != 0 {
                            needs_reorder = true;
                        }

                        // Ignore trailing whitespace for metrics computation
                        // (we are iterating backwards so trailing whitespace comes first)
                        if !have_metrics && line_item.is_whitespace {
                            continue;
                        }

                        // Compute the run's advance by summing the advances of it's constituant clusters
                        line_item.advance = self.layout.clusters[line_item.cluster_range.clone()]
                            .iter()
                            .map(|c| c.advance)
                            .sum();

                        // Compute the run's vertical metrics
                        let run = &self.layout.runs[line_item.index];
                        let line_height = line_item.compute_line_height(self.layout);
                        line.metrics.ascent =
                            line.metrics.ascent.max(run.metrics.ascent * line_height);
                        line.metrics.descent =
                            line.metrics.descent.max(run.metrics.descent * line_height);
                        line.metrics.leading =
                            line.metrics.leading.max(run.metrics.leading * line_height);

                        // Mark us as having seen non-whitespace content on this line
                        have_metrics = true;
                    }
                }
            }

            // Reorder the items within the line (if required). Reordering is required if the line contains
            // a mix of bidi levels (a mix of LTR and RTL text)
            let item_count = line.item_range.end - line.item_range.start;
            if needs_reorder && item_count > 1 {
                reorder_line_items(&mut self.lines.line_items[line.item_range.clone()]);
            }

            // Compute size of line's trailing whitespace
            line.metrics.trailing_whitespace = 0.0;
            if !line.item_range.is_empty() {
                let last_run = &self
                    .lines
                    .line_items
                    .iter()
                    .rfind(|item| item.is_text_run())
                    .unwrap();
                if !last_run.cluster_range.is_empty() {
                    let cluster = &self.layout.clusters[last_run.cluster_range.end - 1];
                    if cluster.info.whitespace().is_space_or_nbsp() {
                        line.metrics.trailing_whitespace = cluster.advance;
                    }
                }
            }

            // Apply alignment to line items
            let has_finite_width = line.max_advance.is_finite() && line.max_advance < f32::MAX;
            if has_finite_width {
                // Compute free space. Alignment only applies if free_space > 0
                let free_space =
                    line.max_advance - line.metrics.advance + line.metrics.trailing_whitespace;

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
                                for line_item in self.lines.line_items[line.item_range.clone()]
                                    .iter()
                                    .filter(|item| item.is_text_run())
                                {
                                    // Iterate over clusters in the run
                                    //   - Iterate forwards for even bidi levels (which represent RTL runs)
                                    //   - Iterate backwards for odd bidi levels (which represent RTL runs)
                                    let clusters =
                                        &mut self.layout.clusters[line_item.cluster_range.clone()];
                                    let bidi_level_is_odd = line_item.bidi_level & 1 != 0;
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
                if !line.item_range.is_empty() {
                    let line_item = &self.lines.line_items[line.item_range.start];
                    if line_item.is_text_run() {
                        let run = &self.layout.runs[line_item.index];
                        line.metrics.ascent = run.metrics.ascent;
                        line.metrics.descent = run.metrics.descent;
                        line.metrics.leading = run.metrics.leading;
                    }
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
        // Compute the overall width and height of the entire layout
        // The "width" excludes trailing whitespace. The "full_width" includes it.
        let mut width = 0f32;
        let mut full_width = 0f32;
        let mut height = 0f32;
        for line in &self.lines.lines {
            width = width.max(line.metrics.advance - line.metrics.trailing_whitespace);
            full_width = full_width.max(line.metrics.advance);
            height += line.metrics.size();
        }

        // Save the computed widths/height to the layout
        self.layout.width = width;
        self.layout.full_width = full_width;
        self.layout.height = height;

        // for (i, line) in self.lines.lines.iter().enumerate() {
        //     println!("LINE {i}");
        //     for item_idx in line.item_range.clone() {
        //         let item = &self.lines.line_items[item_idx];
        //         println!("  ITEM {:?} ({})", item.kind, item.advance);
        //     }
        // }

        // Save the computed lines to the layout
        self.lines.swap(self.layout);
    }
}

/// Removes previous justification applied to clusters.
/// This is part of resetting state in preparation for re-linebreaking the same layout.
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
                for line_run in layout.line_items[line.item_range.clone()]
                    .iter()
                    .filter(|item| item.is_text_run())
                {
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

    // Ensure that the cluster and item endpoints are within range
    state.clusters.end = state.clusters.end.min(layout.clusters.len());
    state.items.end = state.items.end.min(layout.items.len());

    let start_item_idx = lines.line_items.len();
    // let start_run_idx = lines.line_items.last().map(|item| item.index).unwrap_or(0);

    let items_to_commit = &layout.items[state.items.clone()];

    // Compute first and last run index
    let is_text_run = |item: &LayoutItem| item.kind == LayoutItemKind::TextRun;
    let first_run_pos = items_to_commit.iter().position(is_text_run).unwrap_or(0);
    let last_run_pos = items_to_commit.iter().rposition(is_text_run).unwrap_or(0);

    // // Return if line contains no runs
    // let (Some(first_run_pos), Some(last_run_pos)) = (first_run_pos, last_run_pos) else {
    //     return false;
    // };

    //let runs = &layout.runs[state.runs.clone()];
    // let start_run_idx = items_to_commit[first_run_pos].index;
    // let end_run_idx = items_to_commit[last_run_pos].index;

    // Iterate over the items to commit
    // println!("\nCOMMIT LINE");
    for (i, item) in items_to_commit.iter().enumerate() {
        // println!("i = {} index = {} {:?}", i, item.index, item.kind);

        match item.kind {
            LayoutItemKind::InlineBox => {
                let inline_box = &layout.inline_boxes[item.index];

                lines.line_items.push(LineItemData {
                    kind: LayoutItemKind::InlineBox,
                    index: item.index,
                    bidi_level: item.bidi_level,
                    advance: inline_box.width,

                    // These properties are ignored for inline boxes. So we just put a dummy value.
                    is_whitespace: false,
                    has_trailing_whitespace: false,
                    cluster_range: 0..0,
                    text_range: 0..0,
                });
            }
            LayoutItemKind::TextRun => {
                let run_data = &layout.runs[item.index];

                // Compute cluster range
                // The first and last ranges have overrides to account for line-breaks within runs
                let mut cluster_range = run_data.cluster_range.clone();
                if i == first_run_pos {
                    cluster_range.start = state.clusters.start;
                }
                if i == last_run_pos {
                    cluster_range.end = state.clusters.end;
                }

                if cluster_range.start > cluster_range.end
                    || (!is_empty && cluster_range.start == cluster_range.end)
                {
                    // println!("INVALID CLUSTER");
                    // dbg!(&run_data.text_range);
                    // dbg!(cluster_range);
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

                lines.line_items.push(LineItemData {
                    kind: LayoutItemKind::TextRun,
                    index: item.index,
                    bidi_level: run_data.bidi_level,
                    advance: 0.,
                    is_whitespace: false,
                    has_trailing_whitespace: false,
                    cluster_range,
                    text_range,
                });
            }
        }
    }
    // let end_run_idx = lines.line_items.last().map(|item| item.index).unwrap_or(0);
    let end_item_idx = lines.line_items.len();

    // Return false and don't commit line if there were no items to process
    // FIXME: support lines with only inlines boxes
    if start_item_idx == end_item_idx {
        // } || first_run_pos == last_run_pos {
        return false;
    }

    // Q: why this special case?
    let mut num_spaces = state.num_spaces;
    if break_reason == BreakReason::Regular {
        num_spaces = num_spaces.saturating_sub(1);
    }

    lines.lines.push(LineData {
        // run_range: start_run_idx..end_run_idx,
        item_range: start_item_idx..end_item_idx,
        max_advance,
        alignment,
        break_reason,
        num_spaces,
        metrics: LineMetrics {
            advance: state.x,
            ..Default::default()
        },
        ..Default::default()
    });

    // Reset state for the new line
    state.clusters.start = state.clusters.end;
    state.clusters.end += 1;
    state.items.start = state.items.end - 1;
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

    // Iterate over bidi levels
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
