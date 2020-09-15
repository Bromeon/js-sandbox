// Copyright (c) 2020 Jan Haller. zlib/libpng license.

use std::path::Path;

use deno_core::{CoreIsolate, CoreIsolateState, ErrBox, StartupData, ZeroCopyBuf};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::JsValue;

/// Represents a single JavaScript file that can be executed.
///
/// The code can be loaded from a file or from a string in memory.
/// A typical usage pattern is to load a file with one or more JS function definitions, and then call those functions from Rust.
pub struct Script {
	isolate: CoreIsolate,
	last_rid: u32,
}

impl Script {
	const DEFAULT_FILENAME: &'static str = "sandboxed.js";

	/// Initialize a script with the given JavaScript source code
	///
	/// Returns a new object on success, and an error in case of syntax or initialization error with the code.
	pub fn from_string(js_code: &str) -> Result<Script, ErrBox> {
		// console.log() is not available by default -- add the most basic version with single argument (and no warn/info/... variants)
		let all_code = "const console = { log: function(expr) { Deno.core.print(expr + '\\n', false); } };".to_string() + js_code;

		Self::create_script(&all_code, Self::DEFAULT_FILENAME)
	}

	/// Initialize a script by loading it from a .js file
	///
	/// Returns a new object on success. Fails if the file cannot be opened or in case of syntax or initialization error with the code.
	pub fn from_file(file: impl AsRef<Path>) -> Result<Script, ErrBox> {
		let filename = file.as_ref().file_name().and_then(|s| s.to_str()).unwrap_or(Self::DEFAULT_FILENAME).to_owned();

		match std::fs::read_to_string(file) {
			Ok(js_code) => {
				Self::create_script(&js_code, &filename)
			}
			Err(e) => {
				Err(ErrBox::from(e))
			}
		}
	}

	/// Invokes a JavaScript function.
	///
	/// Passes a single argument `args` to JS by serializing it to JSON (using serde_json).
	/// Multiple arguments are currently not supported, but can easily be emulated using a `Vec` to work as a JSON array.
	pub fn call<P, R>(&mut self, fn_name: &str, args: &P) -> Result<R, ErrBox>
		where P: Serialize, R: DeserializeOwned
	{
		let json_args = serde_json::to_value(args)?;
		let json_result = self.call_json(fn_name, &json_args)?;
		let result: R = serde_json::from_value(json_result)?;

		Ok(result)
	}

	pub(crate) fn call_json(&mut self, fn_name: &str, args: &JsValue) -> Result<JsValue, ErrBox> {
		// Note: ops() is required to initialize internal state
		// Wrap everything in scoped block

		// undefined will cause JSON serialization error, so it needs to be treated as null
		let js_code = format!("{{
			let __rust_result = {f}({a});
			if (typeof __rust_result === 'undefined')
				__rust_result = null;

			Deno.core.ops();
			Deno.core.jsonOpSync(\"__rust_return\", __rust_result);\
		}}", f = fn_name, a = args);

		self.isolate.execute(Self::DEFAULT_FILENAME, &js_code)?;

		let state_rc = CoreIsolate::state(&self.isolate);
		let state = state_rc.borrow();
		let mut table = state.resource_table.borrow_mut();

		// Get resource, and free slot (no longer needed)
		let mut result: Box<JsValue> = table.remove(self.last_rid).unwrap();
		self.last_rid += 1;

		Ok(result.take())
	}

	fn create_script(js_code: &str, js_filename: &str) -> Result<Script, ErrBox> {
		let mut isolate = CoreIsolate::new(StartupData::None, false);
		isolate.execute(js_filename, &js_code)?;
		isolate.register_op_json_sync("__rust_return", Script::op_return);

		Ok(Script { isolate, last_rid: 0 })
	}

	fn op_return(
		state: &mut CoreIsolateState,
		args: JsValue,
		_buf: &mut [ZeroCopyBuf],
	) -> Result<JsValue, ErrBox> {
		let resource_table = &mut state.resource_table.borrow_mut();
		let _rid = resource_table.add("result", Box::new(args));
		//assert_eq!(rid, self.last_rid);

		Ok(serde_json::Value::Null)
	}
}
