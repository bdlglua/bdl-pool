mod address;
mod chart;
mod dom;
mod miners;
mod navigation;
mod pool;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    navigation::init();
    pool::init();
    miners::init();
}
