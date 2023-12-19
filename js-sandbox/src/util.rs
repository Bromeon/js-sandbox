// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

use crate::{JsError, JsValue, Script};

/// Evaluates a standalone Javascript expression, and returns the result as a JSON value.
///
/// If there is an error, Err will be returned.
/// This function is primarily useful for small standalone experiments. Usually, you would want to use the [`Script`](struct.Script.html) struct
/// for more sophisticated Rust->JS interaction.
pub fn eval_json(js_expr: &str) -> Result<JsValue, JsError> {
    println!("eval_json: {}", js_expr);
	let code = format!(
		"
		function __rust_expr() {{
			return ({js_expr});
		}}
	"
	);

	let mut script = Script::from_string(&code)?;
    println!("eval_json: script: {:?}", script);
	script.call_json("__rust_expr", &JsValue::Null)
}

pub fn eval(js_expr: &str) -> Result<(), JsError> {
    println!("eval: {}", js_expr);
    let code = format!(
        "
        function __rust_expr() {{
            {js_expr};
        }}
    "
    );

    let mut script = Script::from_string(&code)?;
    script.call_json("__rust_expr", &JsValue::Null)?;

    Ok(())
}
