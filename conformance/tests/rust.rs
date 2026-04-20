// Copyright (C) 2026 Postquant Labs Incorporated
// SPDX-License-Identifier: AGPL-3.0-or-later

//! Generated conformance tests against the Rust in-process runtime.
//!
//! One `#[test]` per vector under `conformance/vectors/`. See
//! `build.rs` for generation details.

#![cfg(feature = "rust")]

include!(concat!(env!("OUT_DIR"), "/vector_tests_rust.rs"));
