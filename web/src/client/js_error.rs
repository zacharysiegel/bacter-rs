use wasm_bindgen::JsValue;

use shared::error::AppError;

pub fn get_js_error(message: &str, error: &JsValue) -> AppError {
    AppError::new(&format!("{message}; [error={error:?}]"))
}

/// `fields` are `key=value` pairs, which share the one bracketed group with the error.
pub fn get_js_error_with_fields(message: &str, fields: &str, error: &JsValue) -> AppError {
    AppError::new(&format!("{message}; [{fields} error={error:?}]"))
}
