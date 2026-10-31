//! The engine for the site, compiled to WebAssembly: `wasm-pack build web --target no-modules
//! --release --out-dir ../site/pkg --out-name dama_web` (docs/web-api.md). Every call takes and
//! returns JSON strings and only passes through to `engine::Session`, so `dama api` gives the same
//! answers natively. Panics are logged to the browser console (`console_error_panic_hook`); the
//! engine checks its input so that none should happen.

use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
extern "C" {
    /// `performance.now()`: WebAssembly has no `std::time::Instant`. Unlike `Date.now()` it is
    /// monotonic and finer than a millisecond, so it can stop a search with a time budget.
    #[wasm_bindgen(js_namespace = performance)]
    fn now() -> f64;
}

/// Milliseconds, from the browser's clock in WebAssembly and from `Instant` natively: the clock
/// of the session's reports and of its agents' time budgets.
fn clock() -> f64 {
    #[cfg(target_arch = "wasm32")]
    {
        now()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        engine::native_clock()
    }
}

/// `{"engine": "<git describe at build time>", "specs": [...]}`.
#[wasm_bindgen]
pub fn version() -> String {
    console_error_panic_hook::set_once();
    engine::version(env!("DAMA_VERSION")).to_string()
}

/// One game and its agents (`engine::Session`).
#[wasm_bindgen]
pub struct Session {
    inner: engine::Session,
}

impl Default for Session {
    fn default() -> Session {
        Session::new()
    }
}

#[wasm_bindgen]
impl Session {
    /// The start position, White to move, no agents.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Session {
        console_error_panic_hook::set_once();
        Session {
            inner: engine::Session::new(clock),
        }
    }

    pub fn reset(&mut self, board: &str, turn: &str) -> String {
        self.inner.reset(board, turn).to_string()
    }

    pub fn state(&self) -> String {
        self.inner.state().to_string()
    }

    pub fn legal(&self) -> String {
        self.inner.legal().to_string()
    }

    pub fn play(&mut self, mv: &str) -> String {
        self.inner.play(mv).to_string()
    }

    pub fn undo(&mut self) -> String {
        self.inner.undo().to_string()
    }

    pub fn set_agent(&mut self, side: &str, spec: &str, seed: u32) -> String {
        self.inner.set_agent(side, spec, seed).to_string()
    }

    pub fn think(&mut self, side: &str, options: &str) -> String {
        self.inner.think(side, options).to_string()
    }

    pub fn analyze(&mut self, spec: &str, options: &str) -> String {
        self.inner.analyze(spec, options).to_string()
    }

    pub fn eval(&self, weights: &str, options: &str) -> String {
        self.inner.eval(weights, options).to_string()
    }

    pub fn position(&self, options: &str) -> String {
        self.inner.position(options).to_string()
    }
}
