// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Hierarchical tree based style application.
#[cfg(not(feature = "std"))]
use alloc::vec;

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
    // text: String,
    tree: Vec<StyleTreeNode<B>>,
    flatted_styles: Vec<RangedStyle<B>>,
    current_span: usize,
    total_text_len: usize,
    text_last_pushed_at: usize,
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
            current_span: usize::MAX,
            total_text_len: usize::MAX,
            text_last_pushed_at: 0,
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
        self.total_text_len = 0;
        self.text_last_pushed_at = 0;
    }

    pub fn push_style_span(&mut self, style: ResolvedStyle<B>) {
        if self.total_text_len > self.text_last_pushed_at {
            let range = self.text_last_pushed_at..(self.total_text_len);
            let style = self.current_style();
            self.flatted_styles.push(RangedStyle { style, range });
            self.text_last_pushed_at = self.total_text_len;
        }

        self.tree
            .push(StyleTreeNode::span(Some(self.current_span), style));
        self.current_span = self.tree.len() - 1;
    }

    pub fn push_style_modification_span(
        &mut self,
        properties: impl Iterator<Item = ResolvedProperty<B>>,
    ) {
        let mut new_style = self.current_style();
        for prop in properties {
            new_style.apply(prop.clone());
        }

        if self.total_text_len > self.text_last_pushed_at {
            let range = self.text_last_pushed_at..(self.total_text_len);
            let style = self.current_style();
            self.flatted_styles.push(RangedStyle { style, range });
            self.text_last_pushed_at = self.total_text_len;
        }

        self.tree
            .push(StyleTreeNode::span(Some(self.current_span), new_style));
        self.current_span = self.tree.len() - 1;
    }

    pub fn pop_style_span(&mut self) {
        if self.total_text_len > self.text_last_pushed_at {
            let range = self.text_last_pushed_at..(self.total_text_len);
            let style = self.current_style();
            self.flatted_styles.push(RangedStyle { style, range });
            self.text_last_pushed_at = self.total_text_len;
        }

        self.current_span = self.tree[self.current_span]
            .parent
            .expect("Popped root style");
    }

    /// Pushes a property that covers the specified range of text.
    pub fn push_text(&mut self, len: usize) {
        if len == 0 {
            return;
        }

        let start = self.total_text_len;
        let end = self.total_text_len + len;

        self.tree
            .push(StyleTreeNode::text(self.current_span, Range { start, end }));
        self.total_text_len = end;
    }

    /// Computes the sequence of ranged styles.
    pub fn finish(&mut self, styles: &mut Vec<RangedStyle<B>>) {
        if self.total_text_len == usize::MAX {
            self.current_span = usize::MAX;
            self.tree.clear();
            return;
        }

        while let Some(_) = self.tree[self.current_span].parent {
            self.pop_style_span();
        }

        if self.total_text_len > self.text_last_pushed_at {
            let range = self.text_last_pushed_at..(self.total_text_len);
            let style = self.current_style();
            self.flatted_styles.push(RangedStyle { style, range });
            self.text_last_pushed_at = self.total_text_len;
        }

        // println!("FINISH TREE");
        // dbg!(self.total_text_len);
        // dbg!(&self.tree);
        // for span in &self.flatted_styles {
        //     println!("{:?} weight:{}", span.range, span.style.font_weight);
        // }
        // dbg!(&self.flatted_styles);

        styles.clear();
        styles.extend_from_slice(&self.flatted_styles);
    }
}
