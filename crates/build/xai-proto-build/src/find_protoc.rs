use anyhow::{Context, bail};
use std::env;
#[cfg(windows)]
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

/// True when `path` looks like a Windows PE image (`MZ` header).
/// The in-repo DotSlash wrapper is a shebang + JSON text file.
#[cfg(windows)]
fn is_pe_executable(path: &Path) -> bool {
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut magic = [0u8; 2];
    file.read_exact(&mut magic).is_ok() && magic == *b"MZ"
}

fn check_protoc_good(protoc: &Path) -> anyhow::Result<()> {
    let output = Command::new(protoc)
        .arg("--version")
        .output()
        .context("Failed to execute protoc")?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "protoc --version failed, likely dotslash is missing; \
             try `cargo install dotslash`; stdout: {stdout:?}, stderr: {stderr:?}"
        );
    }
    Ok(())
}

fn is_github_actions() -> bool {
    env::var_os("GITHUB_ACTIONS").is_some()
}

/// Find `protoc` command.
///
/// Search order:
/// 1. `$PROTOC` environment variable (set by Bazel `build_script_env` or user override)
/// 2. `bin/protoc` walking up parent directories (dotslash wrapper for local dev)
/// 3. `protoc` on `$PATH` (system install or other tooling)
///
/// When `bin/protoc` exists but fails to execute (e.g. the dotslash wrapper running
/// in Bazel remote execution where `dotslash` is not installed), the error is not fatal —
/// we fall through to the PATH-based lookup instead.
///
/// Returns `Ok(None)` if not found and not in a strict environment (GitHub Actions).
pub fn find_protoc() -> anyhow::Result<Option<PathBuf>> {
    // 1. Check the PROTOC env var first. This is the standard override used by prost-build
    //    and is set by Bazel cargo_build_script build_script_env to point at a hermetic
    //    protoc binary instead of the dotslash wrapper.
    if let Ok(protoc_env) = env::var("PROTOC") {
        let protoc = PathBuf::from(&protoc_env);
        if protoc.try_exists()? {
            check_protoc_good(&protoc)?;
            return Ok(Some(protoc));
        }
    }

    // 2. Walk up directories looking for bin/protoc (dotslash wrapper).
    let cwd = env::current_dir()?;
    let mut dir = cwd.clone();
    let mut dir_rel = PathBuf::new();
    loop {
        // Return relative path to make build more deterministic.
        let protoc = dir_rel.join("bin/protoc");
        if protoc.try_exists()? {
            // The in-repo `bin/protoc` is a DotSlash wrapper (shebang + JSON).
            // It has no Windows platform, and CreateProcess rejects the text
            // file with ERROR_BAD_EXE_FORMAT (os error 193). Skip straight to
            // PATH / $PROTOC rather than attempting to execute it.
            #[cfg(windows)]
            if !is_pe_executable(&protoc) {
                eprintln!(
                    "bin/protoc at `{}` is not a Windows executable \
                     (DotSlash wrapper); trying protoc from PATH as fallback",
                    protoc.display()
                );
                break;
            }
            match check_protoc_good(&protoc) {
                Ok(()) => return Ok(Some(protoc)),
                Err(e) => {
                    // bin/protoc exists but can't execute — likely the dotslash wrapper
                    // in an environment without dotslash (e.g. Bazel remote execution).
                    // Fall through to PATH-based lookup below.
                    eprintln!(
                        "bin/protoc found at `{}` but failed to execute: {e:#}; \
                         trying protoc from PATH as fallback",
                        protoc.display()
                    );
                    break;
                }
            }
        }
        if !dir.pop() {
            break;
        }
        dir_rel.push("..");
    }

    // 3. Try protoc from PATH (system install or other tooling).
    if check_protoc_good(Path::new("protoc")).is_ok() {
        return Ok(Some(PathBuf::from("protoc")));
    }

    // 4. Not found anywhere.
    if is_github_actions() {
        return Err(anyhow::anyhow!(
            "`protoc` not found (checked $PROTOC env, bin/protoc, and PATH)"
        ));
    }
    eprintln!("`protoc` not found; likely it is missing in docker image");
    Ok(None)
}
