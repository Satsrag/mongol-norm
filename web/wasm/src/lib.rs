//! Browser bindings for the existing Hudum word APIs.

use mongol_norm::Shaper;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Engine {
    shaper: Shaper,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Engine {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            shaper: Shaper::default(),
        }
    }

    pub fn shape(&self, text: &str) -> Result<String, JsValue> {
        self.shaper
            .shape_str(text)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub fn normalize(&self, text: &str) -> Result<String, JsValue> {
        self.shaper
            .normalize(text)
            .map_err(|error| JsValue::from_str(&error.to_string()))
    }

    pub fn version(&self) -> String {
        mongol_norm::version().to_owned()
    }

    pub fn canonical_version(&self) -> String {
        self.shaper
            .canonical_version()
            .unwrap_or_default()
            .to_owned()
    }
}
