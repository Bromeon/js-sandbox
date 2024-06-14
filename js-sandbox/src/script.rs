// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt::{Debug, Formatter};
use std::path::Path;
use std::rc::Rc;
use std::{thread, time::Duration};

use deno_core::anyhow::Context;
use deno_core::v8::{Global, Value};
use deno_core::{op2, serde_v8, v8, Extension, FastString, JsBuffer, JsRuntime, Op, OpState};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::{AnyError, CallArgs, JsError, JsValue};

pub trait JsApi<'a> {
	/// Generate an API from a script
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
	added_namespaces: BTreeMap<String, Global<Value>>,
}

impl Debug for Script {
	fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
		f.debug_struct("Script")
			.field("runtime", &"...")
			.field("last_rid", &self.last_rid)
			.field("timeout", &self.timeout)
			.finish()
	}
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum CallResult<R> {
	Error { error: String },
	Result(R),
}

impl Script {
	const DEFAULT_FILENAME: &'static str = "sandboxed.js";

	// ----------------------------------------------------------------------------------------------------------------------------------------------
	// Constructors and builders

	/// Initialize a script with the given JavaScript source code.
	///
	/// Returns a new object on success, and an error in case of syntax or initialization error with the code.
	pub fn from_string(js_code: &str) -> Result<Self, JsError> {
		// console.log() is not available by default -- add the most basic version with single argument (and no warn/info/... variants)
		let all_code =
			"const console = { log: function(expr) { Deno.core.print(expr + '\\n', false); } };"
				.to_string() + js_code;

		Self::create_script(all_code)
	}

	/// Initialize a script by loading it from a .js file.
	///
	/// To load a file at compile time, you can use [`Self::from_string()`] in combination with the [`include_str!`] macro.
	/// At the moment, a script is limited to a single file, and you will need to do bundling yourself (e.g. with `esbuild`).
	///
	/// Returns a new object on success. Fails if the file cannot be opened or in case of syntax or initialization error with the code.
	pub fn from_file(file: impl AsRef<Path>) -> Result<Self, JsError> {
		match std::fs::read_to_string(file) {
			Ok(js_code) => Self::create_script(js_code),
			Err(e) => Err(JsError::Runtime(AnyError::from(e))),
		}
	}

	pub fn new() -> Self {
		let ext = Extension {
			ops: Cow::Owned(vec![op_return::DECL]),
			..Default::default()
		};

		let runtime = JsRuntime::new(deno_core::RuntimeOptions {
			module_loader: Some(Rc::new(deno_core::FsModuleLoader)),
			extensions: vec![ext],
			..Default::default()
		});

		Script {
			runtime,
			last_rid: 0,
			timeout: None,
			added_namespaces: Default::default(),
		}
	}

	pub fn add_script(
		&mut self,
		namespace: &str,
		fn_name: &str,
		js_code: &str,
	) -> Result<(), JsError> {
		if self.added_namespaces.contains_key(namespace) {
			return Ok(());
		}

		let js_code = format!(
			"
			var {namespace} = (function() {{
				{js_code}

				return {{
					{fn_name}: function (input) {{
						try {{
							return {fn_name}(input)
						}} catch (e) {{
							return {{ error: `${{e}}` }}
						}}
					}}
				}}
			}})();
			{namespace}.{fn_name}
		"
		);

		// We cannot provide a dynamic filename because execute_script() requires a &'static str
		let global = self
			.runtime
			.execute_script(Self::DEFAULT_FILENAME, js_code.into())?;

		self.added_namespaces.insert(namespace.to_string(), global);

		Ok(())
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
	pub fn call<A, R>(&mut self, fn_name: &str, args_tuple: A) -> Result<R, JsError>
	where
		A: CallArgs,
		R: DeserializeOwned,
	{
		let json_args = args_tuple.into_arg_string()?;
		let json_result = self.call_impl(None, fn_name, json_args)?;
		let result: R = serde_json::from_value(json_result)?;

		Ok(result)
	}

	pub fn call_namespace<A, R>(&mut self, namespace: &str, arg: A) -> Result<R, JsError>
	where
		A: Serialize,
		R: DeserializeOwned,
	{
		deno_core::futures::executor::block_on(self.runtime.run_event_loop(Default::default()))?;

		let Some(global) = self.added_namespaces.get(namespace) else {
			return Err(JsError::Runtime(AnyError::msg(
				"Failed to get namespace function",
			)));
		};
		let scope = &mut self.runtime.handle_scope();
		let scope = &mut v8::HandleScope::new(scope);
		let input = serde_v8::to_v8(scope, arg).with_context(|| "Could not serialize arg")?;
		let local = v8::Local::new(scope, global);
		let func = v8::Local::<v8::Function>::try_from(local)
			.with_context(|| "Could not create function out of local")?;
		let Some(func_res) = func.call(scope, local, &[input]) else {
			return Err(JsError::Runtime(AnyError::msg("Failed to call func")));
		};
		let deserialized_value = serde_v8::from_v8::<serde_json::Value>(scope, func_res)
			.with_context(|| "Could not serialize func res")?;
		let sanitized_value = Self::sanitize_number(deserialized_value)?;
		let result: CallResult<R> = serde_json::from_value(sanitized_value)?;
		match result {
			CallResult::Error { error } => Err(JsError::Runtime(AnyError::msg(error))),
			CallResult::Result(r) => Ok(r),
		}
	}

	fn sanitize_number(value: serde_json::Value) -> Result<serde_json::Value, JsError> {
		match value {
			serde_json::Value::Number(number) => {
				if number.is_f64() {
					let f = number.as_f64().ok_or_else(|| {
						JsError::Runtime(AnyError::msg("Failed to convert number to f64"))
					})?;

					if f.fract() == 0.0 {
						return Ok(serde_json::Value::Number(serde_json::Number::from(
							f as i64,
						)));
					}

					Ok(serde_json::Value::Number(
						serde_json::Number::from_f64(f).ok_or_else(|| {
							JsError::Runtime(AnyError::msg("Failed to convert f64 to number"))
						})?,
					))
				} else if number.is_u64() {
					Ok(serde_json::Value::Number(
						number
							.as_i64()
							.ok_or_else(|| {
								JsError::Runtime(AnyError::msg("Failed to convert number to i64"))
							})?
							.into(),
					))
				} else if number.is_i64() {
					Ok(serde_json::Value::Number(number))
				} else {
					Err(JsError::Runtime(AnyError::msg("Failed to convert number")))
				}
			}
			serde_json::Value::Object(map) => {
				let mut new_map = serde_json::Map::new();
				for (key, value) in map {
					new_map.insert(key, Self::sanitize_number(value)?);
				}
				Ok(serde_json::Value::Object(new_map))
			}
			serde_json::Value::Array(vec) => {
				let mut new_vec = Vec::new();
				for value in vec {
					new_vec.push(Self::sanitize_number(value)?);
				}
				Ok(serde_json::Value::Array(new_vec))
			}
			_ => Ok(value),
		}
	}

	pub fn bind_api<'a, A>(&'a mut self) -> A
	where
		A: JsApi<'a>,
	{
		A::from_script(self)
	}

	pub(crate) fn call_json(&mut self, fn_name: &str, args: &JsValue) -> Result<JsValue, JsError> {
		self.call_impl(None, fn_name, args.to_string())
	}

	fn call_impl(
		&mut self,
		namespace: Option<&str>,
		fn_name: &str,
		json_args: String,
	) -> Result<JsValue, JsError> {
		deno_core::futures::executor::block_on(self.call_impl_async(namespace, fn_name, json_args))
	}

	async fn call_impl_async(
		&mut self,
		namespace: Option<&str>,
		fn_name: &str,
		json_args: String,
	) -> Result<JsValue, JsError> {
		// Note: ops() is required to initialize internal state
		// Wrap everything in scoped block

		let fn_name = if let Some(namespace) = namespace {
			Cow::Owned(format!("{namespace}.{fn_name}"))
		} else {
			Cow::Borrowed(fn_name)
		};

		// 'undefined' will cause JSON serialization error, so it needs to be treated as null
		let js_code = format!(
			"(async () => {{
				let __rust_result = {fn_name}.constructor.name === 'AsyncFunction'
					? await {fn_name}({json_args})
					: {fn_name}({json_args});

				if (typeof __rust_result === 'undefined')
					__rust_result = null;

				Deno.core.ops.op_return(__rust_result);
			}})()"
		)
		.into();

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

		self.runtime.run_event_loop(Default::default()).await?;

		let state_rc = self.runtime.op_state();
		let mut state = state_rc.borrow_mut();
		let table = &mut state.resource_table;

		// Get resource, and free slot (no longer needed)
		let entry: Result<Rc<ResultResource>, deno_core::anyhow::Error> = table.take(self.last_rid);

		match entry {
			Ok(entry) => {
				let extracted = Rc::try_unwrap(entry);

				if extracted.is_err() {
					return Err(JsError::Runtime(AnyError::msg(
						"Failed to unwrap resource entry",
					)));
				}

				let extracted = extracted.unwrap();

				self.last_rid += 1;

				Ok(extracted.json_value)
			}
			Err(e) => Err(JsError::Runtime(AnyError::from(e))),
		}
	}

	fn create_script<S>(js_code: S) -> Result<Self, JsError>
	where
		S: Into<FastString>,
	{
		let mut script = Self::new();
		script
			.runtime
			.execute_script(Self::DEFAULT_FILENAME, js_code.into())?;
		Ok(script)
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

#[op2]
#[serde]
fn op_return(
	state: &mut OpState,
	#[serde] args: JsValue,
	#[buffer] _buf: Option<JsBuffer>,
) -> Result<JsValue, deno_core::error::AnyError> {
	let entry = ResultResource { json_value: args };
	let resource_table = &mut state.resource_table;
	let _rid = resource_table.add(entry);
	Ok(serde_json::Value::Null)
}
