// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hierarchical tree based style application.
use alloc::borrow::Cow;
#[cfg(not(feature = "std"))]
use alloc::vec;

use crate::style::WhiteSpaceCollapse;

use super::*;
use core::ops::Range;

#[derive(Debug, Clone)]
struct StyleTreeNode<B: Brush> {
    parent: Option<usize>,
    data: StyleTreeNodeData<B>,
}

impl<B: Brush> StyleTreeNode<B> {
    fn span(parent: Option<usize>, style: ResolvedStyle<B>) -> Self {
        StyleTreeNode {
            parent,
            data: StyleTreeNodeData::Span(StyleSpan { style }),
        }
    }
    fn text(parent: usize, text_range: Range<usize>) -> Self {
        StyleTreeNode {
            parent: Some(parent),
            data: StyleTreeNodeData::Text(TextSpan { text_range }),
        }
    }
}

#[derive(Debug, Clone)]
enum StyleTreeNodeData<B: Brush> {
    Span(StyleSpan<B>),
    Text(TextSpan),
}

impl<B: Brush> StyleTreeNodeData<B> {
    fn as_span(&self) -> Option<&StyleSpan<B>> {
        match self {
            StyleTreeNodeData::Span(ref span) => Some(span),
            StyleTreeNodeData::Text(_) => None,
        }
    }
}

#[derive(Debug, Clone)]
struct StyleSpan<B: Brush> {
    style: ResolvedStyle<B>,
}

#[derive(Debug, Clone)]
struct TextSpan {
    text_range: Range<usize>,
}

/// Builder for constructing a tree of styles
#[derive(Clone)]
pub struct TreeStyleBuilder<B: Brush> {
    tree: Vec<StyleTreeNode<B>>,
    flatted_styles: Vec<RangedStyle<B>>,
    white_space_collapse: WhiteSpaceCollapse,
    text: String,
    uncomitted_text: String,
    current_span: usize,
    is_span_first: bool,
}

impl<B: Brush> TreeStyleBuilder<B> {
    fn current_style(&self) -> ResolvedStyle<B> {
        self.tree[self.current_span]
            .data
            .as_span()
            .unwrap()
            .style
            .clone()
    }
}

impl<B: Brush> Default for TreeStyleBuilder<B> {
    fn default() -> Self {
        Self {
            tree: Vec::new(),
            flatted_styles: Vec::new(),
            white_space_collapse: WhiteSpaceCollapse::Preserve,
            text: String::new(),
            uncomitted_text: String::new(),
            current_span: usize::MAX,
            is_span_first: false,
        }
    }
}

impl<B: Brush> TreeStyleBuilder<B> {
    /// Prepares the builder for accepting a style tree for text of the specified length.
    pub fn begin(&mut self, root_style: ResolvedStyle<B>) {
        self.tree.clear();
        self.flatted_styles.clear();

        self.tree.push(StyleTreeNode::span(None, root_style));
        self.current_span = 0;
        self.is_span_first = true;
        self.text.clear();
        self.uncomitted_text.clear();
    }

    pub fn set_white_space_mode(&mut self, white_space_collapse: WhiteSpaceCollapse) {
        self.white_space_collapse = white_space_collapse;
    }

    pub fn push_uncomitted_text(&mut self, is_span_last: bool) {
        let span_text: Cow<str> = match self.white_space_collapse {
            WhiteSpaceCollapse::Preserve => Cow::from(&self.uncomitted_text),
            WhiteSpaceCollapse::Collapse => {
                let mut span_text = self.uncomitted_text.as_str();

                if self.is_span_first {
                    span_text = span_text.trim_start();
                }
                if is_span_last {
                    span_text = span_text.trim_end();
                }

                // Collapse spaces
                let mut last_char_whitespace = false;
                let span_text: String = span_text
                    .chars()
                    .filter_map(|c: char| {
                        let this_char_whitespace = c.is_ascii_whitespace();
                        let prev_char_whitespace = last_char_whitespace;
                        last_char_whitespace = this_char_whitespace;

                        if this_char_whitespace {
                            if prev_char_whitespace {
                                None
                            } else {
                                Some(' ')
                            }
                        } else {
                            Some(c)
                        }
                    })
                    .collect();

                Cow::from(span_text)
            }
        };
        let span_text = span_text.as_ref();

        // Nothing to do if there is no uncommited text
        if span_text.len() == 0 {
            return;
        }

        let range = self.text.len()..(self.text.len() + span_text.len());
        let style = self.current_style();
        self.flatted_styles.push(RangedStyle { style, range });
        self.text.push_str(span_text);
        self.uncomitted_text.clear();
        self.is_span_first = false;
    }

    pub fn current_text_len(&self) -> usize {
        self.text.len()
    }

    pub fn push_style_span(&mut self, style: ResolvedStyle<B>) {
        self.push_uncomitted_text(false);

        self.tree
            .push(StyleTreeNode::span(Some(self.current_span), style));
        self.current_span = self.tree.len() - 1;
        self.is_span_first = true;
    }

    pub fn push_style_modification_span(
        &mut self,
        properties: impl Iterator<Item = ResolvedProperty<B>>,
    ) {
        let mut new_style = self.current_style();
        for prop in properties {
            new_style.apply(prop.clone());
        }

        self.push_uncomitted_text(false);

        self.tree
            .push(StyleTreeNode::span(Some(self.current_span), new_style));
        self.current_span = self.tree.len() - 1;
    }

    pub fn pop_style_span(&mut self) {
        self.push_uncomitted_text(true);

        self.current_span = self.tree[self.current_span]
            .parent
            .expect("Popped root style");
    }

    /// Pushes a property that covers the specified range of text.
    pub fn push_text(&mut self, text: &str) {
        if text.len() > 0 {
            self.uncomitted_text.push_str(text);
        }
    }

    /// Computes the sequence of ranged styles.
    pub fn finish(&mut self, styles: &mut Vec<RangedStyle<B>>) -> String {
        while let Some(_) = self.tree[self.current_span].parent {
            self.pop_style_span();
        }

        self.push_uncomitted_text(true);

        // println!("FINISH TREE");
        // dbg!(self.total_text_len);
        // dbg!(&self.tree);
        // for span in &self.flatted_styles {
        //     println!("{:?} weight:{}", span.range, span.style.font_weight);
        // }
        // dbg!(&self.flatted_styles);

        // println!("TEXT");
        // dbg!(&self.text);

        styles.clear();
        styles.extend_from_slice(&self.flatted_styles);

        core::mem::take(&mut self.text)
    }
}
