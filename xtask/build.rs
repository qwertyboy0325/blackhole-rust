//! Emit compile-time Cargo build-profile metadata for Gate 2A0.

fn main() {
    let profile = env_or("PROFILE", "unknown");
    let opt_level = env_or("OPT_LEVEL", "unknown");
    let debug = env_or("DEBUG", "0");
    let target = env_or("TARGET", "unknown");

    println!("cargo:rustc-env=BH_CARGO_PROFILE={profile}");
    println!("cargo:rustc-env=BH_OPT_LEVEL={opt_level}");
    println!("cargo:rustc-env=BH_DEBUG={debug}");
    println!("cargo:rustc-env=BH_TARGET={target}");

    let toolchain = std::process::Command::new("rustc")
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() {
                None
            } else {
                Some(s)
            }
        })
        .unwrap_or_else(|| "unknown".into());
    println!("cargo:rustc-env=BH_TOOLCHAIN={toolchain}");

    // Rebuild when profile-related env changes.
    println!("cargo:rerun-if-env-changed=PROFILE");
    println!("cargo:rerun-if-env-changed=OPT_LEVEL");
    println!("cargo:rerun-if-env-changed=DEBUG");
    println!("cargo:rerun-if-env-changed=TARGET");
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}
