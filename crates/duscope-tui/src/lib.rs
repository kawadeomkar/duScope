//! The interactive viewer behind `duscope view` and `duscope scan -i`.
//!
//! This crate is an implementation detail of the `duscope` binary and promises
//! no API stability before 1.0.

#![forbid(unsafe_code)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
