// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hit testing.

use super::*;

/// Represents a position within a layout.
#[derive(Copy, Clone, Default, Debug)]
pub struct Cursor {
    /// Path to the target cluster.
    pub path: CursorPath,
    /// Offset to the baseline.
    pub baseline: f32,
    /// Offset to the target cluster along the baseline.
    pub offset: f32,
    /// Advance of the target cluster.
    pub advance: f32,
    /// Start of the target cluster.
    pub text_start: usize,
    /// End of the target cluster.
    pub text_end: usize,
    /// Insert point of the cursor (leading or trailing).
    pub insert_point: usize,
    /// `true` if the target cluster is in a right-to-left run.
    pub is_rtl: bool,
    /// `true` if the cursor was created from a point or position inside the layout
    pub is_inside: bool,
}

impl Cursor {
    /// Creates a new cursor from the specified layout and point.
    pub fn from_point<B: Brush>(layout: &Layout<B>, mut x: f32, y: f32) -> Self {
        let mut result = Self {
            is_inside: x >= 0. && y >= 0.,
            ..Default::default()
        };
        let last_line = layout.data.lines.len().saturating_sub(1);
        for (line_index, line) in layout.lines().enumerate() {
            let line_metrics = line.metrics();
            if y > line_metrics.baseline + line_metrics.descent + line_metrics.leading * 0.5 {
                if line_index != last_line {
                    continue;
                }
                result.is_inside = false;
                x = f32::MAX;
            } else if y < 0. {
                x = 0.;
            }
            result.baseline = line_metrics.baseline;
            result.path.line_index = line_index;
            let mut last_edge = line_metrics.offset;
            for (run_index, run) in line.runs().enumerate() {
                result.path.run_index = run_index;
                for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                    let range = cluster.text_range();
                    result.text_start = range.start;
                    result.text_end = range.end;
                    result.is_rtl = run.is_rtl();
                    result.path.cluster_index = run.visual_to_logical(cluster_index).unwrap();
                    if x >= last_edge {
                        let advance = cluster.advance();
                        let next_edge = last_edge + advance;
                        result.offset = next_edge;
                        result.insert_point = range.end;
                        if x >= next_edge {
                            last_edge = next_edge;
                            continue;
                        }
                        result.advance = advance;
                        if x <= (last_edge + next_edge) * 0.5 {
                            result.insert_point = range.start;
                            result.offset = last_edge;
                        }
                    } else {
                        result.is_inside = false;
                        result.insert_point = range.start;
                        result.offset = line_metrics.offset;
                    }
                    return result;
                }
            }
            break;
        }
        result.is_inside = false;
        result
    }

    /// Creates a new cursor for the specified layout and text position.
    pub fn from_position<B: Brush>(
        layout: &Layout<B>,
        mut position: usize,
        is_leading: bool,
    ) -> Self {
        let mut result = Self {
            is_inside: true,
            ..Default::default()
        };
        if position >= layout.data.text_len {
            result.is_inside = false;
            position = layout.data.text_len.saturating_sub(1);
        }
        let last_line = layout.data.lines.len().saturating_sub(1);
        for (line_index, line) in layout.lines().enumerate() {
            let line_metrics = line.metrics();
            result.baseline = line_metrics.baseline;
            result.path.line_index = line_index;
            if !line.text_range().contains(&position) && line_index != last_line {
                continue;
            }
            let mut last_edge = line_metrics.offset;
            result.offset = last_edge;
            for (run_index, run) in line.runs().enumerate() {
                result.path.run_index = run_index;
                if !run.text_range().contains(&position) {
                    last_edge += run.advance();
                    result.offset = last_edge;
                    continue;
                }
                for (cluster_index, cluster) in run.visual_clusters().enumerate() {
                    let range = cluster.text_range();
                    result.text_start = range.start;
                    result.text_end = range.end;
                    result.offset = last_edge;
                    result.is_rtl = run.is_rtl();
                    result.path.cluster_index = run.visual_to_logical(cluster_index).unwrap();
                    let advance = cluster.advance();
                    if range.contains(&position) {
                        if !is_leading || !result.is_inside {
                            result.offset += advance;
                        }
                        result.insert_point = if is_leading { range.start } else { range.end };
                        result.advance = advance;
                        return result;
                    }
                    last_edge += advance;
                }
            }
            result.offset = last_edge;
            break;
        }
        result.insert_point = result.text_end;
        result.is_inside = false;
        result
    }

    /// Returns true if the cursor is on the leading edge of the target
    /// cluster.
    pub fn is_leading(&self) -> bool {
        self.text_start == self.insert_point
    }

    /// Returns true if the cursor is on the trailing edge of the target
    /// cluster.
    pub fn is_trailing(&self) -> bool {
        self.text_end == self.insert_point
    }
}

/// Index based path to a cluster.
#[derive(Copy, Clone, Default, Debug)]
pub struct CursorPath {
    /// Index of the containing line.
    pub line_index: usize,
    /// Index of the run within the containing line.
    pub run_index: usize,
    /// Index of the cluster within the containing run.
    pub cluster_index: usize,
}

impl CursorPath {
    /// Returns the line for this path and the specified layout.
    pub fn line<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Line<'a, B>> {
        layout.get(self.line_index)
    }

    /// Returns the run for this path and the specified layout.
    pub fn run<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Run<'a, B>> {
        self.line(layout)?.get(self.run_index)
    }

    /// Returns the cluster for this path and the specified layout.
    pub fn cluster<'a, B: Brush>(&self, layout: &'a Layout<B>) -> Option<Cluster<'a, B>> {
        self.run(layout)?.get(self.cluster_index)
    }
}
