//! Gives the crate the engine's version at build time (`git describe`), for `version()`.

use std::process::Command;

fn main() {
    let describe = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "unknown".to_string());
    println!("cargo:rustc-env=DAMA_VERSION={describe}");
    // A new commit or tag moves HEAD or a ref; changes to the working tree are not seen.
    println!("cargo:rerun-if-changed=../.git/HEAD");
    println!("cargo:rerun-if-changed=../.git/refs");
}
