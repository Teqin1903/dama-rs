//! Helpers shared by the integration tests. Every file in tests/ is compiled as its own crate,
//! so shared code lives in this subdirectory module and each test file declares `mod common;`.

use std::path::Path;

use engine::Color;

/// The contents of `testdata/<file>`.
pub fn read_testdata(file: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../testdata")
        .join(file);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The side to move as the testdata files write it: `"white"` or `"black"`.
pub fn side(name: &str) -> Color {
    match name {
        "white" => Color::White,
        "black" => Color::Black,
        other => panic!("unknown side {other:?}"),
    }
}
