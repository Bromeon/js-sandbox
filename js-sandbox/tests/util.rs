// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

use deno_core::error::JsError;

use js_sandbox::AnyError;

pub fn expect_error<T>(result: Result<T, AnyError>, error_type: &str) {
	let err = match result {
		Ok(_) => panic!("Call with {error_type} must not succeed"),
		Err(e) => e,
	};

	let err = err
		.downcast_ref::<JsError>()
		.unwrap_or_else(|| panic!("{error_type} must lead to JsError type"));

	let msg = err.message.clone().unwrap_or(String::from("unknown"));

	println!("Expected error occurred:\n{msg}");
}
