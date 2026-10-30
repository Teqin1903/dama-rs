//! The committed pkg (site/pkg/dama_web_bg.wasm) must be built from the source as it is: it holds
//! the version build.rs computes from that source (`src.` and the tree of the WebAssembly sources).

#[test]
fn site_pkg_is_built_from_this_source() {
    let version = env!("DAMA_VERSION");
    if version == "src.unknown" {
        eprintln!("no git: the freshness of site/pkg is not checked");
        return;
    }
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../site/pkg/dama_web_bg.wasm");
    let wasm = std::fs::read(path).unwrap_or_else(|e| panic!("{path}: {e}"));
    let found = wasm.windows(version.len()).any(|w| w == version.as_bytes());
    assert!(
        found,
        "site/pkg is stale: run scripts/build-web.sh (the source is {version})"
    );
}
