//! Gives the crate the version of its source for `version()`: `src.` and the first 7 digits of the
//! git tree of the files that go into the WebAssembly build (`SOURCES`). The tree is written from
//! the working files through a temporary index in OUT_DIR, so it is the same before and after a
//! commit, and a pkg built for a commit carries that commit's source. Without git (a build from
//! the submission zip) the version is `src.unknown`. `tests/fresh.rs` looks for it in site/pkg.

use std::path::Path;
use std::process::Command;

/// The files that make the WebAssembly build, relative to the workspace root. Tests, benches and
/// the CLI are left out, so changing them leaves the pkg fresh.
const SOURCES: [&str; 7] = [
    "Cargo.toml",
    "Cargo.lock",
    "engine/Cargo.toml",
    "engine/src",
    "web/Cargo.toml",
    "web/build.rs",
    "web/src",
];

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    for source in SOURCES {
        println!("cargo:rerun-if-changed={}", root.join(source).display());
    }
    let out_dir = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let index = Path::new(&out_dir).join("source-index");
    let version = source_tree(&root, &index)
        .map(|tree| format!("src.{}", &tree[..7]))
        .unwrap_or_else(|| "src.unknown".to_string());
    println!("cargo:rustc-env=DAMA_VERSION={version}");
}

/// The git tree of `SOURCES` as they are on disk, or `None` without git or a repository.
fn source_tree(root: &Path, index: &Path) -> Option<String> {
    let git = |args: &[&str]| {
        let output = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_INDEX_FILE", index)
            .output()
            .ok()?;
        output.status.success().then_some(output.stdout)
    };
    git(&["read-tree", "--empty"])?;
    let mut add = vec!["add", "--"];
    add.extend(SOURCES);
    git(&add)?;
    let tree = String::from_utf8(git(&["write-tree"])?).ok()?;
    let tree = tree.trim();
    (tree.len() >= 7).then(|| tree.to_string())
}
