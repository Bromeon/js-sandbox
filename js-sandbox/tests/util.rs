// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

use js_sandbox::JsError;

pub fn expect_error<T>(result: Result<T, JsError>, error_type: &str) {
	let err = match result {
		Ok(_) => panic!("Call with {error_type} must not succeed"),
		Err(e) => e,
	};

	if let JsError::Runtime(e) = err {
		let err = e
			.downcast_ref::<deno_core::error::JsError>()
			.unwrap_or_else(|| panic!("{error_type} must lead to deno_core::error::JsError type"));
		println!("Expected error occurred:\n{err}");
	}
}
