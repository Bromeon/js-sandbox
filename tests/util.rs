// Copyright (c) 2020-2022 js-sandbox contributors. Zlib license.

use deno_core::error::JsError;

use js_sandbox::AnyError;

pub fn expect_error<T>(result: Result<T, AnyError>, error_type: &str) {
	let err = match result {
		Ok(_) => panic!("Call with {} must not succeed", error_type),
		Err(e) => e,
	};

	let err = err
		.downcast_ref::<JsError>()
		.expect(&format!("{} must lead to JsError type", error_type));

	println!("Expected error occurred:\n{}", err.message);
}
