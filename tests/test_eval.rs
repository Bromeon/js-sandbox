// Copyright (c) 2020-2021 Jan Haller. zlib/libpng license.

use js_sandbox::JsValue;
use util::expect_error;

mod util;

#[test]
fn console_log() {
	let result: JsValue = js_sandbox::eval_json("console.log(\"Hello World\")")
		.expect("Valid expression can be evaluated");

	assert_eq!(result, JsValue::Null);
}

#[test]
fn expression() {
	let result: JsValue = js_sandbox::eval_json("({a: 43, b: 12}).b - 2")
		.expect("Valid expression can be evaluated");

	let exp_result = JsValue::from(10);

	assert_eq!(result, exp_result);
}

#[test]
fn syntax_error() {
	let result_opt = js_sandbox::eval_json("({a: 43, b: 12})..b - 2");

	expect_error(result_opt, "Syntax error");
}
