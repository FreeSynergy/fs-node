// Build script — embeds build timestamp and git hash as compile-time env vars.
// Accessible in code via:  env!("FSN_BUILD_TIME")  and  env!("FSN_GIT_HASH")

fn main() {
    // Build time via OS date command (no external crates needed)
    let build_time = std::process::Command::new("date")
        .arg("+%Y-%m-%d %H:%M")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=FSN_BUILD_TIME={}", build_time);

    // Short git hash for identification
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "?".into());
    println!("cargo:rustc-env=FSN_GIT_HASH={}", hash);

    // Re-run when git HEAD changes (e.g. after commit)
    println!("cargo:rerun-if-changed=.git/HEAD");
}
