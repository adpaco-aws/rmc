// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Ensure we have basic support of portable SIMD.
#![feature(portable_simd)]

use std::simd::{mask32x4, u64x16, ToBitMask};

#[kani::proof]
fn check_sum_any() {
    let a = u64x16::splat(0);
    let b = u64x16::from_array(kani::any());
    // Cannot compare them directly: https://github.com/model-checking/kani/issues/2632
    assert_eq!((a + b).as_array(), b.as_array());
}

#[kani::proof]
fn check_mask() {
    // From array doesn't work either. Manually build [false, true, false, true]
    let mut mask = mask32x4::splat(false);
    mask.set(1, true);
    mask.set(3, true);
    let bitmask = mask.to_bitmask();
    assert_eq!(bitmask, 0b1010);
}
