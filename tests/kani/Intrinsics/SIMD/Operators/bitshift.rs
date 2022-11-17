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
pub struct i64x2(i64, i64);

extern "platform-intrinsic" {
    fn simd_shl<T>(x: T, y: T) -> T;
    fn simd_shr<T>(x: T, y: T) -> T;
}

#[kani::proof]
fn main() {
    let x: i32 = kani::any();
    kani::assume(x == 31);
    // let value: i32 = 4;
    let result: i32 = 1 << x;
    // assert_eq!(result, 0);
    // let values = i64x2(2, 2);
    // let shifts = i64x2(1, 1);
    // let x4 = unsafe { simd_shl(values, shifts) };
    // assert_eq!(x4, i64x2(4, 4))
}
