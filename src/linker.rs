// Native linking for Palladium
// "Generated C that gcc never optimizes is C thrown away"
//
// Every backend path ends the same way: hand gcc the generated .c, the C
// runtime, and an output name. Six call sites used to spell that out by hand
// and *none* of them passed an optimization level, so `pdc` shipped -O0 code
// while `pdc compile -O` was silently ignored. Measured cost on this machine:
// bubble_sort 1714ms -> 286ms, matrix_multiply 1794ms -> 332ms once gcc is
// allowed to optimize the exact same C. This module is the single place that
// answers "how do we invoke gcc?", so a flag can never go missing from one
// path again.

use crate::errors::Result;
use crate::runtime_paths;
use std::path::Path;
use std::process::Command;

/// How hard gcc should work on the generated C.
///
/// Palladium's C backend emits straightforward, unoptimized C (every local is
/// a real store, every call a real call); the optimizer that makes it
/// competitive with rustc lives in gcc. So the *default* is optimized, and
/// turning it off is the explicit choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OptLevel {
    /// `-O0`: for reading/debugging the emitted C, where generated line
    /// structure must survive into the binary.
    None,
    /// `-O2`: the default for every compile.
    #[default]
    Default,
    /// `-O3`: `pdc compile -O` and release package builds.
    Aggressive,
}

impl OptLevel {
    /// The gcc flag for this level.
    pub fn flag(self) -> &'static str {
        match self {
            OptLevel::None => "-O0",
            OptLevel::Default => "-O2",
            OptLevel::Aggressive => "-O3",
        }
    }

    /// Resolve the two CLI booleans into a level. `--no-opt` wins if both are
    /// somehow set, so "I need to debug this" is never overridden by a
    /// leftover `-O` (clap also rejects the combination up front).
    pub fn from_flags(optimize: bool, no_opt: bool) -> Self {
        match (optimize, no_opt) {
            (_, true) => OptLevel::None,
            (true, false) => OptLevel::Aggressive,
            (false, false) => OptLevel::Default,
        }
    }

    /// Package builds: `--release` means `-O3`, everything else the default.
    pub fn for_release(release: bool) -> Self {
        if release {
            OptLevel::Aggressive
        } else {
            OptLevel::Default
        }
    }
}

/// The gcc invocation that turns generated C into an executable.
///
/// Resolves the runtime itself (`-I<dir>` for `pd_prelude.h`, plus
/// `palladium_runtime.c` as a second translation unit) so no call site has to
/// remember either. Returns the command unexecuted: each caller phrases its
/// own failure message ("Linking failed", "Test compilation failed", ...).
pub fn link_command(c_file: &Path, output: &Path, opt: OptLevel) -> Result<Command> {
    let runtime_dir = runtime_paths::runtime_dir()?;
    let runtime_c = runtime_dir.join(runtime_paths::RUNTIME_C_FILE);

    let mut cmd = Command::new("gcc");
    cmd.arg(opt.flag())
        .arg("-I")
        .arg(&runtime_dir)
        .arg(c_file)
        .arg(&runtime_c)
        .arg("-o")
        .arg(output);
    Ok(cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_map_to_gcc_flags() {
        assert_eq!(OptLevel::None.flag(), "-O0");
        assert_eq!(OptLevel::Default.flag(), "-O2");
        assert_eq!(OptLevel::Aggressive.flag(), "-O3");
    }

    /// The regression this module exists for: no flags at all must still be
    /// an optimized build.
    #[test]
    fn default_is_optimized() {
        assert_eq!(OptLevel::default(), OptLevel::Default);
        assert_eq!(OptLevel::from_flags(false, false).flag(), "-O2");
    }

    #[test]
    fn optimize_flag_selects_o3_and_no_opt_wins() {
        assert_eq!(OptLevel::from_flags(true, false), OptLevel::Aggressive);
        assert_eq!(OptLevel::from_flags(false, true), OptLevel::None);
        assert_eq!(OptLevel::from_flags(true, true), OptLevel::None);
    }

    #[test]
    fn release_packages_use_o3() {
        assert_eq!(OptLevel::for_release(true), OptLevel::Aggressive);
        assert_eq!(OptLevel::for_release(false), OptLevel::Default);
    }

    /// The level must actually reach the command line, ahead of the sources.
    #[test]
    fn link_command_carries_the_level_and_the_runtime() {
        let cmd = link_command(
            Path::new("/tmp/x.c"),
            Path::new("/tmp/x"),
            OptLevel::Aggressive,
        )
        .expect("runtime should resolve in a dev checkout");

        let args: Vec<String> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        assert_eq!(cmd.get_program(), "gcc");
        assert_eq!(args[0], "-O3");
        assert!(args.iter().any(|a| a == "-I"), "{:?}", args);
        assert!(
            args.iter()
                .any(|a| a.ends_with(runtime_paths::RUNTIME_C_FILE)),
            "{:?}",
            args
        );
        assert_eq!(args[args.len() - 2], "-o");
        assert_eq!(args[args.len() - 1], "/tmp/x");
    }
}
