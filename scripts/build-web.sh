#!/usr/bin/env bash
# Builds the engine for the site into site/pkg/: dama_web.js and dama_web_bg.wasm, nothing else.
# wasm-pack also writes a .gitignore (with *), package.json and a README; they are removed, since
# the .gitignore would hide every file added to site/pkg/ later. web/tests/fresh.rs checks that
# the committed pkg was built from the source as it is.
set -euo pipefail
cd "$(dirname "$0")/.."

flags=(--target no-modules --release --out-dir ../site/pkg --out-name dama_web --no-typescript)
if wasm-pack build --help | grep -q -- '--no-pack'; then
    flags+=(--no-pack)
fi
wasm-pack build web "${flags[@]}"

find site/pkg -mindepth 1 -maxdepth 1 ! -name dama_web.js ! -name dama_web_bg.wasm -exec rm -rf {} +
ls -l site/pkg
