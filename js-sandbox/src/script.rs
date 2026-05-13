// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

use std::path::Path;
use std::rc::Rc;
use std::{thread, time::Duration};

use deno_core::{serde_v8, v8, FastString, JsRuntime, ModuleLoader, NoopModuleLoader};
use serde::de::DeserializeOwned;

use crate::{CallArgs, JsError, JsValue};

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
	timeout: Option<Duration>,
}

impl Script {
	const DEFAULT_FILENAME: &'static str = "sandboxed.js";

	// ----------------------------------------------------------------------------------------------------------------------------------------------
	// Constructors and builders

	/// Initialize a script with the given JavaScript source code.
	///
	/// Returns a new object on success, and an error in case of syntax or initialization error with the code.
	///
	/// The script runs with a deny-all module loader: dynamic `import(...)` from JavaScript is rejected.
	/// Use [`Self::from_string_with_loader()`] to provide a custom [`ModuleLoader`].
	pub fn from_string(js_code: &str) -> Result<Self, JsError> {
		Self::create_script(js_code.to_string(), Rc::new(NoopModuleLoader))
	}

	/// Initialize a script by loading it from a .js file.
	///
	/// To load a file at compile time, you can use [`Self::from_string()`] in combination with the [`include_str!`] macro.
	/// At the moment, a script is limited to a single file, and you will need to do bundling yourself (e.g. with `esbuild`).
	///
	/// Returns a new object on success. Fails if the file cannot be opened or in case of syntax or initialization error with the code.
	///
	/// As with [`Self::from_string()`], dynamic `import(...)` from JavaScript is rejected.
	pub fn from_file(file: impl AsRef<Path>) -> Result<Self, JsError> {
		match std::fs::read_to_string(file) {
			Ok(js_code) => Self::create_script(js_code, Rc::new(NoopModuleLoader)),
			Err(e) => Err(JsError::from(e)),
		}
	}

	/// Initialize a script with a custom [`ModuleLoader`], enabling JavaScript `import(...)`.
	///
	/// # Security
	/// The default constructors deliberately reject all dynamic imports, because `deno_core`'s built-in
	/// [`deno_core::FsModuleLoader`] reads arbitrary host files and exposes them to JS via import attributes
	/// such as `{ with: { type: "text" } }`, fully bypassing the sandbox.
	///
	/// Passing a loader here re-enables that mechanism. The caller is responsible for ensuring the loader
	/// only resolves trusted specifiers. **Do not pass `FsModuleLoader` if the JS code is untrusted.**
	pub fn from_string_with_loader(
		js_code: &str,
		module_loader: Rc<dyn ModuleLoader>,
	) -> Result<Self, JsError> {
		Self::create_script(js_code.to_string(), module_loader)
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

	pub(crate) fn call_json(&mut self, fn_name: &str, args: &JsValue) -> Result<JsValue, JsError> {
		self.call_impl(fn_name, args.to_string())
	}

	fn call_impl(&mut self, fn_name: &str, json_args: String) -> Result<JsValue, JsError> {
		// Wrap call in async IIFE so async + sync functions resolve uniformly via the event loop.
		let js_code: FastString = format!("(async () => {fn_name}({json_args}))()").into();

		if let Some(timeout) = self.timeout {
			let handle = self.runtime.v8_isolate().thread_safe_handle();

			thread::spawn(move || {
				thread::sleep(timeout);
				handle.terminate_execution();
			});
		}

		let promise = self
			.runtime
			.execute_script(Self::DEFAULT_FILENAME, js_code)?;

		let resolved = deno_core::futures::executor::block_on(async {
			let fut = self.runtime.resolve(promise);
			self.runtime
				.with_event_loop_promise(fut, deno_core::PollEventLoopOptions::default())
				.await
		})?;

		deno_core::scope!(scope, &mut self.runtime);
		let local = v8::Local::new(scope, resolved);
		let value: JsValue = serde_v8::from_v8(scope, local)?;
		Ok(value)
	}

	fn create_script<S>(js_code: S, module_loader: Rc<dyn ModuleLoader>) -> Result<Self, JsError>
	where
		S: Into<FastString>,
	{
		let mut runtime = JsRuntime::new(deno_core::RuntimeOptions {
			module_loader: Some(module_loader),
			..Default::default()
		});

		// We cannot provide a dynamic filename because execute_script() requires a &'static str
		runtime.execute_script(Self::DEFAULT_FILENAME, js_code.into())?;

		Ok(Script {
			runtime,
			timeout: None,
		})
	}
}
