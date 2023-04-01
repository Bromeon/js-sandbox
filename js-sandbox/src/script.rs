// Copyright (c) 2020-2022 js-sandbox contributors. Zlib license.

use std::borrow::Cow;
use std::path::Path;
use std::rc::Rc;
use std::{thread, time::Duration};

use deno_core::{op, Extension, JsRuntime, OpState, ZeroCopyBuf};
use serde::de::DeserializeOwned;

use crate::call_args::CallArgs;
use crate::{AnyError, JsValue};

pub trait JsApi<'a> {
	fn from_script(script: &'a mut Script) -> Self
	where
		Self: Sized;
}

/// Represents a single JavaScript file that can be executed.
///
/// The code can be loaded from a file or from a string in memory.
/// A typical usage pattern is to load a file with one or more JS function definitions, and then call those functions from Rust.
pub struct Script {
	runtime: JsRuntime,
	last_rid: u32,
	timeout: Option<Duration>,
}

impl Script {
	const DEFAULT_FILENAME: &'static str = "sandboxed.js";

	// ----------------------------------------------------------------------------------------------------------------------------------------------
	// Constructors and builders

	/// Initialize a script with the given JavaScript source code.
	///
	/// Returns a new object on success, and an error in case of syntax or initialization error with the code.
	pub fn from_string(js_code: &str) -> Result<Self, AnyError> {
		// console.log() is not available by default -- add the most basic version with single argument (and no warn/info/... variants)
		let all_code =
			"const console = { log: function(expr) { Deno.core.print(expr + '\\n', false); } };"
				.to_string() + js_code;

		Self::create_script(&all_code, Self::DEFAULT_FILENAME)
	}

	/// Initialize a script by loading it from a .js file.
	///
	/// To load a file at compile time, you can use [`Self::from_string()`] in combination with the [`include_str!`] macro.
	/// At the moment, a script is limited to a single file, and you will need to do bundling yourself (e.g. with `esbuild`).
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

	/// Equips this script with a timeout, meaning that any function call is aborted after the specified duration.
	///
	/// This requires creating a separate thread for each function call, which tracks time and pulls the plug
	/// if the JS function does not return in time. Use this for untrusted 3rd-party code, not if you know that
	/// your functions always return.
	///
	/// Panics with invalid timeouts or if this script already has a timeout set.
	pub fn with_timeout(mut self, timeout: Duration) -> Self {
		assert!(self.timeout.is_none());
		assert!(timeout > Duration::ZERO);

		self.timeout = Some(timeout);
		self
	}

	// ----------------------------------------------------------------------------------------------------------------------------------------------
	// Call API

	/// Invokes a JavaScript function.
	///
	/// Blocks on asynchronous functions until completion.
	///
	/// `args_tuple` needs to be a tuple.
	///
	/// Each tuple element is converted to JSON (using serde_json) and passed as a distinct argument to the JS function.
	pub fn call<A, R>(&mut self, fn_name: &str, args_tuple: A) -> Result<R, AnyError>
	where
		A: CallArgs,
		R: DeserializeOwned,
	{
		let json_args = args_tuple.into_arg_string()?;
		let json_result = self.call_impl(fn_name, json_args)?;
		let result: R = serde_json::from_value(json_result)?;

		Ok(result)
	}

	pub fn bind_api<'a, A>(&'a mut self) -> A
	where
		A: JsApi<'a>,
	{
		A::from_script(self)
	}

	pub(crate) fn call_json(&mut self, fn_name: &str, args: &JsValue) -> Result<JsValue, AnyError> {
		self.call_impl(fn_name, args.to_string())
	}

	fn call_impl(&mut self, fn_name: &str, json_args: String) -> Result<JsValue, AnyError> {
		// Note: ops() is required to initialize internal state
		// Wrap everything in scoped block

		// 'undefined' will cause JSON serialization error, so it needs to be treated as null
		let js_code = format!(
			"(async () => {{
				let __rust_result = {f}.constructor.name === 'AsyncFunction' ? await {f}({a}) : {f}({a});
				if (typeof __rust_result === 'undefined')
					__rust_result = null;

				Deno.core.ops.op_return(__rust_result);
			}})()",
			f = fn_name,
			a = json_args
		);

		if let Some(timeout) = self.timeout {
			let handle = self.runtime.v8_isolate().thread_safe_handle();

			thread::spawn(move || {
				thread::sleep(timeout);
				handle.terminate_execution();
			});
		}

		// syncing ops is required cause they sometimes change while preparing the engine
		// self.runtime.sync_ops_cache();

		// TODO use strongly typed JsError here (downcast)
		self.runtime
			.execute_script(Self::DEFAULT_FILENAME, js_code)?;
		deno_core::futures::executor::block_on(self.runtime.run_event_loop(false))?;

		let state_rc = self.runtime.op_state();
		let mut state = state_rc.borrow_mut();
		let table = &mut state.resource_table;

		// Get resource, and free slot (no longer needed)
		let entry: Rc<ResultResource> = table
			.take(self.last_rid)
			.expect("Resource entry must be present");
		let extracted =
			Rc::try_unwrap(entry).expect("Rc must hold single strong ref to resource entry");
		self.last_rid += 1;

		Ok(extracted.json_value)
	}

	fn create_script(js_code: &str, js_filename: &str) -> Result<Self, AnyError> {
		let ext = Extension::builder("script")
			.ops(vec![(op_return::decl())])
			.build();

		let mut runtime = deno_core::JsRuntime::new(deno_core::RuntimeOptions {
			module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
			extensions: vec![ext],
			..Default::default()
		});
		runtime.execute_script(Self::DEFAULT_FILENAME, js_code.to_string())?;

		Ok(Script {
			runtime,
			last_rid: 0,
			timeout: None,
		})
	}
}

#[derive(Debug)]
struct ResultResource {
	json_value: JsValue,
}

// Type that is stored inside Deno's resource table
impl deno_core::Resource for ResultResource {
	fn name(&self) -> Cow<str> {
		"__rust_Result".into()
	}
}

#[op]
fn op_return(
	state: &mut OpState,
	args: JsValue,
	_buf: Option<ZeroCopyBuf>,
) -> Result<JsValue, deno_core::error::AnyError> {
	let entry = ResultResource { json_value: args };
	let resource_table = &mut state.resource_table;
	let _rid = resource_table.add(entry);
	Ok(serde_json::Value::Null)
}
