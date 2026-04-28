// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

use js_sandbox::JsError;

pub fn expect_error<T>(result: Result<T, JsError>, error_type: &str) {
	let err = match result {
		Ok(_) => panic!("Call with {error_type} must not succeed"),
		Err(e) => e,
	};

	match &err {
		JsError::Runtime(_) => println!("Expected error occurred:\n{err}"),
		other => panic!("{error_type} must lead to a JS runtime error, got: {other:?}"),
	}
}
