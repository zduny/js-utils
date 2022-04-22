use std::fmt::Display;

use wasm_bindgen::prelude::*;
use web_sys::Window;

pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
    pub fn warn(s: &str);
    pub fn error(s: &str);
}

#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => {
        {
            use crate::utils::log;
            (log(&format_args!($($t)*).to_string()))
        }
    }
}

#[macro_export]
macro_rules! console_warn {
    ($($t:tt)*) => {
        {
            use crate::utils::warn;
            (warn(&format_args!($($t)*).to_string()))
        }
    }
}

#[macro_export]
macro_rules! console_error {
    ($($t:tt)*) => {
        {
            use crate::utils::error;
            (error(&format_args!($($t)*).to_string()))
        }
    }
}

#[macro_export]
macro_rules! closure {
    ($expression:expr) => {{
        use wasm_bindgen::prelude::Closure;
        Closure::wrap(Box::new($expression) as Box<dyn FnMut(_)>)
    }};
}

pub fn window() -> Window {
    web_sys::window().unwrap()
}

#[derive(Debug)]
pub struct JsError {
    value: JsValue,
}

impl Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.value)
    }
}

impl std::error::Error for JsError {}

impl From<JsValue> for JsError {
    fn from(value: JsValue) -> Self {
        JsError { value }
    }
}
