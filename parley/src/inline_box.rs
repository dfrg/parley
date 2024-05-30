// Copyright 2024 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/// A box to be laid out inline with text
#[derive(Debug, Clone)]
pub struct InlineBox {
    /// User-specified identifier for the box, which can be used by the user to determine which box in
    /// parley's output corresponds to which box in it's input.
    pub id: u64,
    /// The character index into the underlying text string at which the box should be placed.
    pub index: usize,
    /// The width of the box in pixels
    pub width: f32,
    /// The height of the box in pixels
    pub height: f32,
}
