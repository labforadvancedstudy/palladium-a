//! Shared helpers for the integration test binaries.
//!
//! # Why every test needs its own module name
//!
//! The code generator always writes to `build_output/<module-stem>.c`
//! (`src/codegen/mod.rs:3051-3062`), where the stem comes from the *virtual*
//! filename handed to the driver — not from the source file's real location.
//! Two tests that pass the same name therefore write and re-read the same file.
//!
//! Cargo runs the tests inside one binary on parallel threads, and the test
//! binaries themselves in parallel processes, so those writes interleave. That
//! was measurable: `tests/compiler_comprehensive_test.rs` compiled everything
//! as `"test.pd"`, `tests/advanced_e2e_test.rs` and
//! `tests/advanced_features_test.rs` both wrote their source to `test.pd` in a
//! temp dir (which the driver then turned back into `build_output/test.c`), and
//! the suite's failure count moved between runs.
//!
//! A gate whose result depends on thread scheduling cannot be read at all, so
//! every test that compiles something routes its name through here.

use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A module name that is unique across every test in every test binary of a
/// run: the process id separates the binaries, the counter separates the
/// threads inside one binary.
///
/// `prefix` should identify the test, so that a leftover `build_output/*.c`
/// can still be traced back to the test that produced it.
#[allow(dead_code)]
pub fn unique_module_name(prefix: &str) -> String {
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{}_{}_{}", prefix, std::process::id(), n)
}

/// The same, with the `.pd` extension the driver expects for a source name.
#[allow(dead_code)]
pub fn unique_source_name(prefix: &str) -> String {
    format!("{}.pd", unique_module_name(prefix))
}
