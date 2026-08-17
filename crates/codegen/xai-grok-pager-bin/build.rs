use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-env-changed=GROK_VERSION");

    // Windows defaults the main-thread stack to 1 MiB (PE SizeOfStackReserve).
    // Debug clap parsing of PagerArgs plus the pager startup future overflow
    // that immediately (`thread 'main' has overflowed its stack`, even for
    // `grok version`). Match the 8 MiB session-actor stack. Only the final
    // pager link is affected — not the rest of the crate graph.
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if os == "windows" {
        const STACK: &str = "8388608";
        let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        if env == "msvc" {
            println!("cargo:rustc-link-arg=/STACK:{STACK}");
        } else {
            println!("cargo:rustc-link-arg=-Wl,--stack,{STACK}");
        }
    }

    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let version = std::env::var("GROK_VERSION")
        .or_else(|_| std::env::var("CARGO_PKG_VERSION"))
        .unwrap_or_else(|_| "0.0.0".to_string());

    println!(
        "cargo:rustc-env=VERSION_WITH_COMMIT={} ({})",
        version, commit
    );
}
