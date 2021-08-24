// Copyright (c) 2020-2021 Jan Haller. zlib/libpng license.

use crate::{AnyError, JsValue, Script};

/// Evaluates a standalone Javascript expression, and returns the result as a JSON value.
///
/// If there is an error, Err will be returned.
/// This function is primarily useful for small standalone experiments. Usually, you would want to use the [`Script`](struct.Script.html) struct
/// for more sophisticated Rust->JS interaction.
/// Optional value for `timeout` forces script to run no more than specified number of milliseconds
pub fn eval_json(js_expr: &str, timeout: Option<u64>) -> Result<JsValue, AnyError> {
	let code = format!("
		function __rust_expr() {{
			return ({expr});
		}}
	", expr = js_expr);

	let mut script = Script::from_string(&code)?;
	script.call_json("__rust_expr", &JsValue::Null, timeout)
}
