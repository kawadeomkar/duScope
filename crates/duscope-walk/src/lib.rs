//! The duscope scanner: a work-stealing directory walker and the per-platform
//! providers that fill its entry records.
//!
//! This crate is an implementation detail of the `duscope` binary and promises
//! no API stability before 1.0.

// Denied rather than forbidden so that `src/platform/` alone can opt back in
// for its FFI; every other crate forbids `unsafe` outright.
#![deny(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
