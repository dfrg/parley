use super::{Alignment, BreakReason, LayoutData};
use crate::style::Brush;

pub(crate) fn align<B: Brush>(
    layout: &mut LayoutData<B>,
    alignment_width: Option<f32>,
    alignment: Alignment,
) {
    let alignment_width = alignment_width.unwrap_or_else(|| {
        let max_line_length = layout
            .lines
            .iter()
            .map(|line| line.metrics.advance)
            .max_by(f32::total_cmp)
            .unwrap_or(0.0);
        max_line_length.min(max_line_length)
    });

    // Apply alignment to line items
    for line in &mut layout.lines {
        // TODO: remove this field
        line.alignment = alignment;

        // Compute free space.
        let free_space = alignment_width - line.metrics.advance + line.metrics.trailing_whitespace;

        // Alignment only applies if free_space > 0
        if free_space <= 0. {
            continue;
        }

        match alignment {
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
                if line.break_reason == BreakReason::None || line.num_spaces == 0 {
                    continue;
                }

                let adjustment = free_space / line.num_spaces as f32;
                let mut applied = 0;
                for line_item in layout.line_items[line.item_range.clone()]
                    .iter()
                    .filter(|item| item.is_text_run())
                {
                    // Iterate over clusters in the run
                    //   - Iterate forwards for even bidi levels (which represent RTL runs)
                    //   - Iterate backwards for odd bidi levels (which represent RTL runs)
                    let clusters = &mut layout.clusters[line_item.cluster_range.clone()];
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

/// Removes previous justification applied to clusters.
/// This is part of resetting state in preparation for re-linebreaking the same layout.
pub(crate) fn unjustify<B: Brush>(layout: &mut LayoutData<B>) {
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
