// Copyright (c) 2020-2021 Jan Haller. zlib/libpng license.

use std::borrow::Cow;
use std::path::Path;
use std::rc::Rc;
use std::{thread, time::Duration};

use deno_core::{JsRuntime, OpState, RuntimeOptions, ZeroCopyBuf};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::{AnyError, JsValue};

/// Represents a single JavaScript file that can be executed.
///
/// The code can be loaded from a file or from a string in memory.
/// A typical usage pattern is to load a file with one or more JS function definitions, and then call those functions from Rust.
pub struct Script {
	runtime: JsRuntime,
	last_rid: u32,
}

impl Script {
	const DEFAULT_FILENAME: &'static str = "sandboxed.js";

	/// Initialize a script with the given JavaScript source code
	///
	/// Returns a new object on success, and an error in case of syntax or initialization error with the code.
	pub fn from_string(js_code: &str) -> Result<Self, AnyError> {
		// console.log() is not available by default -- add the most basic version with single argument (and no warn/info/... variants)
		let all_code = "const console = { log: function(expr) { Deno.core.print(expr + '\\n', false); } };".to_string() + js_code;

		Self::create_script(&all_code, Self::DEFAULT_FILENAME)
	}

	/// Initialize a script by loading it from a .js file
	///
	/// Returns a new object on success. Fails if the file cannot be opened or in case of syntax or initialization error with the code.
	pub fn from_file(file: impl AsRef<Path>) -> Result<Self, AnyError> {
		let filename = file
			.as_ref()
			.file_name()
			.and_then(|s| s.to_str())
			.unwrap_or(Self::DEFAULT_FILENAME)
			.to_owned();

		match std::fs::read_to_string(file) {
			Ok(js_code) => Self::create_script(&js_code, &filename),
			Err(e) => Err(AnyError::from(e)),
		}
	}

	/// Invokes a JavaScript function.
	///
	/// Passes a single argument `args` to JS by serializing it to JSON (using serde_json).
	/// Multiple arguments are currently not supported, but can easily be emulated using a `Vec` to work as a JSON array.
	/// Optional value for `timeout` forces script to run no more than specified number of milliseconds
	pub fn call<P, R>(&mut self, fn_name: &str, args: &P, timeout: Option<u64>) -> Result<R, AnyError>
	where
		P: Serialize,
		R: DeserializeOwned,
	{
		let json_args = serde_json::to_value(args)?;
		let json_result = self.call_json(fn_name, &json_args, timeout)?;
		let result: R = serde_json::from_value(json_result)?;

		Ok(result)
	}

	pub(crate) fn call_json(&mut self, fn_name: &str, args: &JsValue, timeout: Option<u64>) -> Result<JsValue, AnyError> {
		// Note: ops() is required to initialize internal state
		// Wrap everything in scoped block

		// undefined will cause JSON serialization error, so it needs to be treated as null
		let js_code = format!("{{
			let __rust_result = {f}({a});
			if (typeof __rust_result === 'undefined')
				__rust_result = null;

			Deno.core.ops();
			Deno.core.opSync(\"__rust_return\", __rust_result);\
		}}", f = fn_name, a = args);

		if let Some(timeout_duration) = timeout {
			let handle = self.runtime.v8_isolate().thread_safe_handle();

			thread::spawn(move || {
				thread::sleep(Duration::from_millis(timeout_duration));
				handle.terminate_execution();
			});
		}

		self.runtime.execute(Self::DEFAULT_FILENAME, &js_code)?;

		let state_rc = self.runtime.op_state();
		let mut state = state_rc.borrow_mut();
		let table = &mut state.resource_table;

		// Get resource, and free slot (no longer needed)
		let entry: Rc<ResultResource> = table.take(self.last_rid).expect("Resource entry must be present");
		let extracted = Rc::try_unwrap(entry).expect("Rc must hold single strong ref to resource entry");
		self.last_rid += 1;

		Ok(extracted.json_value)
	}

	fn create_script(js_code: &str, js_filename: &str) -> Result<Self, AnyError> {
		let options = RuntimeOptions::default();

		let mut runtime = JsRuntime::new(options);
		runtime.execute(js_filename, &js_code)?;
		runtime.register_op("__rust_return", deno_core::op_sync(Self::op_return));

		Ok(Script { runtime, last_rid: 0 })
	}

	fn op_return(
		state: &mut OpState,
		args: JsValue,
		_buf: Option<ZeroCopyBuf>,
	) -> Result<JsValue, AnyError> {
		let entry = ResultResource { json_value: args };
		let resource_table = &mut state.resource_table;
		let _rid = resource_table.add(entry);
		//assert_eq!(rid, self.last_rid);

		Ok(serde_json::Value::Null)
	}
}

#[derive(Debug)]
struct ResultResource {
	json_value: JsValue
}

// Type that is stored inside Deno's resource table
impl deno_core::Resource for ResultResource {
	fn name(&self) -> Cow<str> {
		"__rust_Result".into()
	}
}