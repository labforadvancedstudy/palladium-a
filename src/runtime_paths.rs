// Runtime asset location for Palladium
// "A compiler that only works from its own source tree is not installed"
//
// Every gcc invocation in this compiler needs two things from the runtime
// directory: `palladium_runtime.c` (linked in) and `pd_prelude.h` (included by
// C emitted from the Palladium-written bootstrap compiler, hence `-I<dir>`).
// Hardcoding `runtime/palladium_runtime.c` made both cwd-dependent, so an
// installed `pdc` could only link from the repo root. This module is the single
// place that answers "where is my runtime?".

use crate::errors::{CompileError, Result};
use std::path::{Path, PathBuf};

/// Environment variable that overrides runtime discovery. Points at the runtime
/// DIRECTORY, not the .c file.
pub const RUNTIME_ENV: &str = "PALLADIUM_RUNTIME";

/// The C runtime translation unit that every generated program links against.
pub const RUNTIME_C_FILE: &str = "palladium_runtime.c";

/// Why resolution failed. Kept separate from `CompileError` so the pure
/// resolution logic stays testable without touching the process environment.
#[derive(Debug, PartialEq, Eq)]
enum ResolveFailure {
    /// `$PALLADIUM_RUNTIME` was set but does not hold the runtime.
    EnvWithoutRuntime(PathBuf),
    /// Nothing was found; carries every candidate that was probed, in order.
    NotFound(Vec<PathBuf>),
}

impl ResolveFailure {
    fn into_error(self) -> CompileError {
        match self {
            ResolveFailure::EnvWithoutRuntime(dir) => CompileError::Generic(format!(
                "{} is set to '{}', but that directory does not contain {}",
                RUNTIME_ENV,
                dir.display(),
                RUNTIME_C_FILE
            )),
            ResolveFailure::NotFound(tried) => {
                let mut msg = format!(
                    "could not locate the Palladium runtime ({} not found). Tried:",
                    RUNTIME_C_FILE
                );
                for path in &tried {
                    msg.push_str(&format!("\n  - {}", path.display()));
                }
                msg.push_str(&format!(
                    "\nSet {} to the directory holding {}.",
                    RUNTIME_ENV, RUNTIME_C_FILE
                ));
                CompileError::Generic(msg)
            }
        }
    }
}

/// The resolved runtime directory. Pass this to gcc as `-I<dir>`.
pub fn runtime_dir() -> Result<PathBuf> {
    let env = std::env::var_os(RUNTIME_ENV)
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok().or(Some(exe)))
        .and_then(|exe| exe.parent().map(Path::to_path_buf));
    let cwd = std::env::current_dir().ok();

    resolve_with(
        env.as_deref(),
        exe_dir.as_deref(),
        cwd.as_deref(),
        Path::new(env!("CARGO_MANIFEST_DIR")),
    )
    .map_err(ResolveFailure::into_error)
}

/// The C runtime file to hand to gcc, inside the resolved runtime directory.
pub fn runtime_c() -> Result<PathBuf> {
    Ok(runtime_dir()?.join(RUNTIME_C_FILE))
}

/// Pure resolution: first hit wins.
///
/// 1. `$PALLADIUM_RUNTIME` (hard error if set but wrong — an explicit override
///    that silently falls through would hide packaging mistakes)
/// 2. next to the executable: `../share/palladium/runtime` (Homebrew layout),
///    `../lib/palladium/runtime`, `./runtime`
/// 3. `<cwd>/runtime` (the historical repo-root workflow)
/// 4. `<CARGO_MANIFEST_DIR>/runtime` (dev checkout, `cargo run` from anywhere)
fn resolve_with(
    env: Option<&Path>,
    exe_dir: Option<&Path>,
    cwd: Option<&Path>,
    manifest_dir: &Path,
) -> std::result::Result<PathBuf, ResolveFailure> {
    if let Some(dir) = env {
        if holds_runtime(dir) {
            return Ok(dir.to_path_buf());
        }
        return Err(ResolveFailure::EnvWithoutRuntime(dir.to_path_buf()));
    }

    let mut tried = Vec::new();

    if let Some(exe_dir) = exe_dir {
        for suffix in [
            "../share/palladium/runtime",
            "../lib/palladium/runtime",
            "runtime",
        ] {
            tried.push(exe_dir.join(suffix));
        }
    }
    if let Some(cwd) = cwd {
        tried.push(cwd.join("runtime"));
    }
    tried.push(manifest_dir.join("runtime"));

    for candidate in &tried {
        if holds_runtime(candidate) {
            // Normalize away the `..` hops so error messages and
            // `--print-runtime` show a path a human can paste.
            return Ok(candidate
                .canonicalize()
                .unwrap_or_else(|_| candidate.clone()));
        }
    }

    Err(ResolveFailure::NotFound(tried))
}

fn holds_runtime(dir: &Path) -> bool {
    dir.join(RUNTIME_C_FILE).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// A directory that looks like a real runtime install.
    fn runtime_fixture(root: &Path, rel: &str) -> PathBuf {
        let dir = root.join(rel);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(RUNTIME_C_FILE), "// runtime\n").unwrap();
        fs::write(dir.join("pd_prelude.h"), "// prelude\n").unwrap();
        dir
    }

    #[test]
    fn env_var_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let env_dir = runtime_fixture(tmp.path(), "custom");
        // A cwd runtime also exists, to prove the env var takes priority.
        let cwd = tmp.path().join("work");
        runtime_fixture(&cwd, "runtime");

        let got = resolve_with(
            Some(&env_dir),
            None,
            Some(&cwd),
            Path::new("/nonexistent/manifest"),
        )
        .unwrap();
        assert_eq!(got, env_dir);
    }

    #[test]
    fn env_var_without_runtime_file_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let empty = tmp.path().join("empty");
        fs::create_dir_all(&empty).unwrap();
        // A perfectly good cwd runtime exists; the bad override must NOT fall
        // through to it, or a broken package would silently link the wrong C.
        let cwd = tmp.path().join("work");
        runtime_fixture(&cwd, "runtime");

        let err = resolve_with(
            Some(&empty),
            None,
            Some(&cwd),
            Path::new("/nonexistent/manifest"),
        )
        .unwrap_err();
        assert_eq!(err, ResolveFailure::EnvWithoutRuntime(empty.clone()));

        let msg = err.into_error().to_string();
        assert!(msg.contains(RUNTIME_ENV), "{}", msg);
        assert!(msg.contains(RUNTIME_C_FILE), "{}", msg);
    }

    #[test]
    fn falls_back_to_cwd_runtime() {
        let tmp = tempfile::tempdir().unwrap();
        let cwd = tmp.path().join("repo");
        let expected = runtime_fixture(&cwd, "runtime");
        // exe_dir has no runtime next to it, so the cwd rule must fire.
        let exe_dir = tmp.path().join("elsewhere/bin");
        fs::create_dir_all(&exe_dir).unwrap();

        let got = resolve_with(
            None,
            Some(&exe_dir),
            Some(&cwd),
            Path::new("/nonexistent/manifest"),
        )
        .unwrap();
        assert_eq!(got, expected.canonicalize().unwrap());
    }

    #[test]
    fn prefers_homebrew_share_layout_next_to_exe() {
        let tmp = tempfile::tempdir().unwrap();
        let prefix = tmp.path().join("prefix");
        let exe_dir = prefix.join("bin");
        fs::create_dir_all(&exe_dir).unwrap();
        let expected = runtime_fixture(&prefix, "share/palladium/runtime");
        runtime_fixture(&prefix, "lib/palladium/runtime");
        // A cwd runtime exists too; install layout must win over cwd.
        let cwd = tmp.path().join("anywhere");
        runtime_fixture(&cwd, "runtime");

        let got = resolve_with(
            None,
            Some(&exe_dir),
            Some(&cwd),
            Path::new("/nonexistent/manifest"),
        )
        .unwrap();
        assert_eq!(got, expected.canonicalize().unwrap());
    }

    #[test]
    fn nothing_found_lists_every_candidate() {
        let tmp = tempfile::tempdir().unwrap();
        let exe_dir = tmp.path().join("bin");
        fs::create_dir_all(&exe_dir).unwrap();
        let cwd = tmp.path().join("cwd");
        fs::create_dir_all(&cwd).unwrap();

        let err = resolve_with(
            None,
            Some(&exe_dir),
            Some(&cwd),
            Path::new("/nonexistent/manifest"),
        )
        .unwrap_err();
        match &err {
            ResolveFailure::NotFound(tried) => assert_eq!(tried.len(), 5),
            other => panic!("expected NotFound, got {:?}", other),
        }

        let msg = err.into_error().to_string();
        assert!(msg.contains("share/palladium/runtime"), "{}", msg);
        assert!(msg.contains(RUNTIME_ENV), "{}", msg);
    }

    /// The real resolver must work in this repo (cwd or manifest rule).
    #[test]
    fn resolves_in_this_checkout() {
        let dir = runtime_dir().expect("runtime should resolve in a dev checkout");
        assert!(dir.join(RUNTIME_C_FILE).is_file(), "{}", dir.display());
        assert_eq!(runtime_c().unwrap(), dir.join(RUNTIME_C_FILE));
    }
}
