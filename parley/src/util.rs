// Copyright 2021 the Parley Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Misc helpers.

pub fn nearly_eq(x: f32, y: f32) -> bool {
    (x - y).abs() < f32::EPSILON
}

pub fn nearly_zero(x: f32) -> bool {
    nearly_eq(x, 0.)
}

// pub enum IterDirection {
//     Forward,
//     Backward,
// }

// impl IterDirection {
//     pub fn from_is_forwards(is_forwards: bool) -> Self {
//         if is_forwards {
//             Self::Forward
//         } else {
//             Self::Backward
//         }
//     }
// }

// #[inline]
// pub fn iter_dyn_direction<T, I: DoubleEndedIterator<Item = T>>(iterator: impl Into<I>, dir: IterDirection, cb: impl FnMut(T)) {
//     let iterator = iterator.into();
//     match dir {
//         IterDirection::Forward => iterator.for_each(cb),
//         IterDirection::Backward => iterator.rev().for_each(cb),
//     }
// }