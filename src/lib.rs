use fluvio_wasm_timer::Delay;
use std::{error::Error, time::Duration};
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

pub async fn wait_a_second() -> Result<(), Box<dyn Error>> {
    wait(Duration::from_secs(1)).await
}

pub async fn wait(duration: Duration) -> Result<(), Box<dyn Error>> {
    Ok(Delay::new(duration).await?)
}
