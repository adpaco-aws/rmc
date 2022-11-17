// Copyright Kani Contributors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Checks that the following SIMD intrinsics are supported:
//!  * `simd_and`
//!  * `simd_or`
//!  * `simd_xor`
//! This is done by initializing vectors with the contents of 2-member tuples
//! with symbolic values. The result of using each of the intrinsics is compared
//! against the result of using the associated bitwise operator on the tuples.
#![feature(repr_simd, platform_intrinsics)]

#[repr(simd)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct i32x2(i32, i32);

extern "platform-intrinsic" {
    fn simd_shl<T>(x: T, y: T) -> T;
    fn simd_shr<T>(x: T, y: T) -> T;
}

#[kani::proof]
fn test_normal() {
    let value: i32 = kani::any();
    let shift: i32 = kani::any();
    kani::assume(value == 1);
    kani::assume(shift == 32);
    let result: i32 = value << shift;
}

#[kani::proof]
fn test_simd() {
    let value = kani::any();
    kani::assume(value == 1);
    let values = i32x2(value, value);
    let shift = kani::any();
    kani::assume(shift == 32);
    let shifts = i32x2(shift, shift);
    let result = unsafe { simd_shl(values, shifts) };
}

// #[kani::proof]
// fn test_normal_lhs_signed() {
//     let shift: i32 = kani::any();
//     kani::assume(shift == 32);
//     let result: i32 = 1 >> shift;
//     // assert_eq!(result, 0);
// }

// #[kani::proof]
// fn test_simd_lhs_signed() {
//     let values = i32x2(1, 1);
//     let shifts = i32x2(kani::any(), kani::any());
//     kani::assume(shifts.0 == 32);
//     kani::assume(shifts.1 == 32);
//     let result = unsafe { simd_shr(values, shifts) };
//     // assert_eq!(x4, i32x2(4, 4))
// }
