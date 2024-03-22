//! Range based style application.

use super::*;
use core::ops::{Bound, Range, RangeBounds};

/// Builder for constructing an ordered sequence of non-overlapping ranged
/// styles from a collection of ranged style properties.
#[derive(Clone)]
pub struct RangedStyleBuilder<B: Brush> {
    properties: Vec<RangedProperty<B>>,
    default_style: ResolvedStyle<B>,
    /// The length of text that the styles apply to
    len: usize,
}

impl<B: Brush> Default for RangedStyleBuilder<B> {
    fn default() -> Self {
        Self {
            properties: vec![],
            default_style: Default::default(),
            len: usize::MAX,
        }
    }
}

impl<B: Brush> RangedStyleBuilder<B> {
    /// Prepares the builder for accepting ranged properties for text of the
    /// specified length.
    pub fn begin(&mut self, len: usize) {
        self.properties.clear();
        self.default_style = ResolvedStyle::default();
        self.len = len;
    }

    /// Pushes a property that covers the full range of text.
    pub fn push_default(&mut self, property: ResolvedProperty<B>) {
        assert!(self.len != usize::MAX);
        self.default_style.apply(property)
    }

    /// Pushes a property that covers the specified range of text.
    pub fn push(&mut self, property: ResolvedProperty<B>, range: impl RangeBounds<usize>) {
        let range = resolve_range(range, self.len);
        assert!(self.len != usize::MAX);
        self.properties.push(RangedProperty { property, range })
    }

    /// Computes the sequence of ranged styles.
    pub fn finish(&mut self, styles: &mut Vec<RangedStyle<B>>) {
        // `usize::MAX` is used as a sentinal value to represent an invalid builder state. So simply return
        // default styles if `finish` is called on a builder in this state
        if self.len == usize::MAX {
            self.properties.clear();
            self.default_style = ResolvedStyle::default();
            return;
        }

        // Push the default style to the resolve list of styles.
        // `styles` is assumed to be empty at the start of this function so we end up with a Vec of length one.
        styles.push(RangedStyle {
            style: self.default_style.clone(),
            range: 0..self.len,
        });

        // Iterate over each ranged property, applying them to the list of styles in turn
        for prop in &self.properties {
            // Skip style property's that have an invalid range (end < start)
            if prop.range.start > prop.range.end {
                continue;
            }

            // Determine which existing ranges new range intersects
            let split_range = split_range(prop, &styles);
            let mut inserted = 0;

            // Split the span that the new range's start point intersects into two spans
            // (unless it starts at exactly the same point as an existing range)
            if let Some(first) = split_range.first {
                // Resolve the span we are splitting from it's index
                let original_span = &mut styles[first];

                // Check if the new styles are actually different to the existing styles for the span we are
                // splitting. If they are not then we can skip the split.
                if !original_span.style.check(&prop.property) {
                    let mut new_span = original_span.clone();
                    let original_end = original_span.range.end;
                    original_span.range.end = prop.range.start;
                    new_span.range.start = prop.range.start;
                    new_span.style.apply(prop.property.clone());

                    // Handle the case where the new range is entirely contained within a single existing span
                    if split_range.replace_len == 0 && split_range.last == Some(first) {
                        let mut new_end_span = original_span.clone();
                        new_end_span.range.start = prop.range.end;
                        new_end_span.range.end = original_end;
                        new_span.range.end = prop.range.end;
                        styles.splice(
                            first + 1..first + 1,
                            [new_span, new_end_span].iter().cloned(),
                        );
                        continue;
                    } else {
                        styles.insert(first + 1, new_span);
                    }
                    inserted += 1;
                }
            }

            // Update all of the ranges that the new range completely encompasses
            let replace_start = split_range.replace_start + inserted;
            let replace_end = replace_start + split_range.replace_len;
            for style in &mut styles[replace_start..replace_end] {
                style.style.apply(prop.property.clone());
            }

            // Split the span that the new range's end point intersects into two spans
            // (unless it starts at exactly the same point as an existing range)
            if let Some(mut last) = split_range.last {
                last += inserted;
                let original_span = &mut styles[last];
                if !original_span.style.check(&prop.property) {
                    let mut new_span = original_span.clone();
                    original_span.range.start = prop.range.end;
                    new_span.range.end = prop.range.end;
                    new_span.style.apply(prop.property.clone());
                    styles.insert(last, new_span);
                }
            }
        }

        // Iterate over all spans of styles, merging consecutive spans if they represent the same styles
        let mut prev_index = 0;
        let mut merged_count = 0;
        for i in 1..styles.len() {
            if styles[prev_index].style == styles[i].style {
                let end = styles[i].range.end;
                styles[prev_index].range.end = end;
                merged_count += 1;
            } else {
                prev_index += 1;
                if prev_index != i {
                    let moved_span = styles[i].clone();
                    styles[prev_index] = moved_span;
                }
            }
        }
        styles.truncate(styles.len() - merged_count);

        self.properties.clear();
        self.default_style = ResolvedStyle::default();
        self.len = usize::MAX;
    }
}

/// Style with an associated range.
#[derive(Clone)]
pub struct RangedStyle<B: Brush> {
    pub style: ResolvedStyle<B>,
    pub range: Range<usize>,
}

#[derive(Clone)]
struct RangedProperty<B: Brush> {
    property: ResolvedProperty<B>,
    range: Range<usize>,
}

/// Struct representing update points for intersecting a range with a slice of non-overlapping ranges to produce
/// and new slice of non-overlapping ranges where the ranges in the original slice have been sub-divided at both
/// the start and end points of the new range (if those are not already the start/end points of an existing range in
/// the slice).
#[derive(Default)]
struct SplitRange {
    /// Represents a span which should be split into two because the new span STARTS within in.
    /// If the start of the new span exactly matches the start of an existing span then this is set to None
    /// as this means that no existing spans need to be split into two as the start of the range
    first: Option<usize>,
    /// Represents a span which should be split into two because the new span ENDS within in.
    /// If the end of the new span exactly matches the end of an existing span then this is set to None
    /// as this means that no existing spans need to be split into two at the end of the range
    last: Option<usize>,

    // The following two properties collectively represent the set of existing spans that should have their
    // styles entirely replaced with new styles matching the new style property
    /// The index of the first span to have it's styles entirely replaced
    replace_start: usize,
    /// The number of spans to have their styles entirely replaced
    replace_len: usize,
}

/// Given a slice of non-overlapping ranges representing resolved styles and a new range that overlaps those ranges
/// ...
fn split_range<B: Brush>(prop: &RangedProperty<B>, spans: &[RangedStyle<B>]) -> SplitRange {
    let mut range = SplitRange::default();

    // Binary search for the existing style span that the new property's START index
    // is contained within, treating spans as an open-closed range.
    let start_span_index =
        match spans.binary_search_by(|span| span.range.start.cmp(&prop.range.start)) {
            Ok(index) => index,
            Err(index) => index.saturating_sub(1),
        };

    // Linearly search for the style span that the new property's END index is contained within,
    // starting from the span that the START index is contained with.
    let mut end_span_index = spans.len() - 1;
    for (i, span) in spans[start_span_index..].iter().enumerate() {
        if span.range.end >= prop.range.end {
            end_span_index = i + start_span_index;
            break;
        }
    }

    // Resolve references to the actual spans from their indices
    let start_span = &spans[start_span_index];
    let end_span = &spans[end_span_index];

    // Check if the START of new property's range exactly matches an existing style span
    // boundary (else case) or if a span needs to be split into two (if case)
    if start_span.range.start < prop.range.start {
        range.first = Some(start_span_index);
        range.replace_start = start_span_index + 1;
    } else {
        range.replace_start = start_span_index;
    }

    // Check if the END of new property's range exactly matches an existing style span
    // boundary (else case) or if a span needs to be split into two (if case)
    if end_span.range.end > prop.range.end {
        range.last = Some(end_span_index);
        range.replace_len = end_span_index.saturating_sub(range.replace_start);
    } else {
        range.replace_len = (end_span_index + 1).saturating_sub(range.replace_start);
    }

    range
}

/// Resolves an arbitary `impl RangeBounds<usize>` into a concrete `Range<usize>`
/// Clamps the resolved range to the range 0..len.
fn resolve_range(range: impl RangeBounds<usize>, len: usize) -> Range<usize> {
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(n) => *n,
        Bound::Excluded(n) => *n + 1,
    };
    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Included(n) => *n + 1,
        Bound::Excluded(n) => *n,
    };
    start.min(len)..end.min(len)
}
