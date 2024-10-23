//! Useful utilities to make development of browser-targeted Rust applications
//! slightly less painful.

#[cfg(feature = "spawn")]
pub mod spawn;
#[cfg(feature = "spawn")]
pub use spawn::spawn;

#[cfg(feature = "sleep")]
pub mod sleep;
#[cfg(feature = "sleep")]
pub use sleep::sleep;

#[cfg(feature = "queue")]
pub mod queue;
#[cfg(feature = "queue")]
pub use queue::Queue;

#[cfg(feature = "event")]
pub mod event;

use std::fmt::Display;

use wasm_bindgen::prelude::*;
use web_sys::{Document, HtmlElement, Window};

/// Sets a panic hook that forwards panic messages to
/// [`console.error`](https://developer.mozilla.org/en-US/docs/Web/API/Console/error).
#[cfg(feature = "panic_hook")]
pub fn set_panic_hook() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
extern "C" {
    /// Outputs a message to the web console.
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);

    /// Outputs a warning message to the web console.
    #[wasm_bindgen(js_namespace = console)]
    pub fn warn(s: &str);

    /// Outputs an error message to the web console.
    #[wasm_bindgen(js_namespace = console)]
    pub fn error(s: &str);
}

/// Macro for [`log`] to add arguments support (like in [`print`] macro).
#[macro_export]
macro_rules! console_log {
    ($($t:tt)*) => {
        {
            ($crate::log(&format_args!($($t)*).to_string()))
        }
    }
}

/// Macro for [`warn`] to add arguments support (like in [`print`] macro).
#[macro_export]
macro_rules! console_warn {
    ($($t:tt)*) => {
        {
            ($crate::warn(&format_args!($($t)*).to_string()))
        }
    }
}

/// Macro for [`error`] to add arguments support (like in [`print`] macro).
#[macro_export]
macro_rules! console_error {
    ($($t:tt)*) => {
        {
            ($crate::error(&format_args!($($t)*).to_string()))
        }
    }
}

/// Helper macro for creating [`mod@wasm_bindgen`] closures.
#[macro_export]
macro_rules! closure {
    ($expression:expr) => {{
        wasm_bindgen::prelude::Closure::wrap(Box::new($expression) as Box<dyn FnMut(_)>)
    }};
}

/// Gets window object.
///
/// This function panics when window doesn't exist.
pub fn window() -> Window {
    web_sys::window().expect("no global window exists")
}

/// Gets document object.
///
/// This function panics when document doesn't exist in window or
/// if window doesn't exist.
pub fn document() -> Document {
    window()
        .document()
        .expect("should have a document on window")
}

/// Gets document's body.
///
/// This function panics when body doesn't exist in document or
/// if document doesn't exist in window or
/// if window doesn't exist.
pub fn body() -> HtmlElement {
    document().body().expect("document should have a body")
}

/// Wrapper for [`JsValue`] errors implementing [`std::error::Error`].
#[derive(Debug)]
pub struct JsError(pub JsValue);

impl Display for JsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl std::error::Error for JsError {}

impl From<JsValue> for JsError {
    fn from(value: JsValue) -> Self {
        JsError(value)
    }
}

#[cfg(test)]
mod tests {
    use crate::{body, document, window};
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    fn test_window() {
        window();
    }

    #[wasm_bindgen_test]
    fn test_body() {
        body();
    }

    #[wasm_bindgen_test]
    fn test_document() {
        document();
    }
}
