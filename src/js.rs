//! Bindings to js
//! TODO: this should be abstracted. Core game should not know anything about js
use once_cell::sync::Lazy;
use std::sync::Mutex;
use std::vec::Vec;
use wasm_bindgen::prelude::*;

pub static DEBUG_LOGS: Lazy<Mutex<Vec<String>>> = Lazy::new(|| Mutex::new(Vec::new()));

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn log_impl(s: &str);
}

pub fn log(s: &str) {
    let mut logs = DEBUG_LOGS.lock().unwrap();
    logs.push(s.to_string());
    log_impl(s);
}

pub fn clear_logs() {
    let mut logs = DEBUG_LOGS.lock().unwrap();
    logs.clear();
}
