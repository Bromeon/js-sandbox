// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

#![allow(clippy::let_unit_value)]

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use js_sandbox::{AnyError, JsError, Script};
use util::expect_error;

mod util;

#[derive(Serialize, Debug)]
struct JsArgs {
	text: String,
	num: i32,
}

#[derive(Deserialize, Debug, PartialEq)]
struct JsResult {
	new_text: String,
	new_num: i32,
}

#[derive(Serialize)]
struct Person {
	name: String,
	age: u8,
}

#[test]
fn call() {
	let src = r#"
	function triple(a) { return 3 * a; }

	function extract(obj) {
		return {
			new_text: obj.text + ".",
			new_num: triple(obj.num)
		};
	}"#;

	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let args = JsArgs {
		text: "hi".to_string(),
		num: 4,
	};
	let exp_result = JsResult {
		new_text: "hi.".to_string(),
		new_num: 12,
	};

	let result: JsResult = script.call("extract", (args,)).unwrap();
	assert_eq!(result, exp_result);
}

#[test]
fn call_multi_args() {
	let src = r#"
	function div(a, b) { return a / b; }
	"#;

	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let args = (15, 4);
	let result: f32 = script.call("div", args).unwrap();
	assert_eq!(result, 3.75);
}

#[test]
fn call_struct_to_string() {
	let src = r#"
	function toString(person) {
		return "A person named " + person.name + " with age " + person.age;
	}"#;

	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let person = Person {
		name: "Roger".to_string(),
		age: 42,
	};
	let result: String = script.call("toString", (person,)).unwrap();

	assert_eq!(result, "A person named Roger with age 42");
}

#[test]
fn call_hashmap_to_hashmap() {
	let src = r#"
	function fillMap(map) {
		map.cats = 2;
		return map;
	}"#;

	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let map = HashMap::from([("dogs", 3)]);
	let result: HashMap<String, i32> = script.call("fillMap", (map,)).unwrap();

	assert_eq!(
		result,
		HashMap::from([("dogs".to_string(), 3), ("cats".to_string(), 2)])
	);
}

#[test]
fn call_minimal() -> Result<(), AnyError> {
	let js_code = "function triple(a) { return 3 * a; }";
	let mut script = Script::from_string(js_code)?;

	let arg = 7;
	let result: i32 = script.call("triple", (arg,))?;

	assert_eq!(result, 21);
	Ok(())
}

#[test]
fn call_void() -> Result<(), AnyError> {
	let js_code = "function print(expr) { console.log(expr); }";
	let mut script = Script::from_string(js_code)?;

	let args = "some text";
	let _result: () = script.call("print", (args,))?;

	Ok(())
}

#[test]
fn call_from_file() {
	let mut script = Script::from_file("tests/hello.js").expect("File can be loaded");

	let args = JsArgs {
		text: "hi".to_string(),
		num: 4,
	};
	let exp_result = JsResult {
		new_text: "hi.".to_string(),
		new_num: 12,
	};

	let result: JsResult = script.call("extract", (args,)).unwrap();
	assert_eq!(result, exp_result);
}

#[test]
fn call_local_state() {
	let src = "var i = 0;
	function inc() { return ++i; }";
	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let args = ();

	// Also test call-by-ref ('args' borrowed)
	let result: i32 = script.call("inc", (&args,)).unwrap();
	assert_eq!(result, 1);

	let result: i32 = script.call("inc", (args,)).unwrap();
	assert_eq!(result, 2);
}

#[test]
fn call_repeated() {
	let src = "
		function triple(a) { return 3 * a; }
		function square(a) { return a * a; }";

	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let args = 7;

	let result_triple: i32 = script.call("triple", (args,)).unwrap();
	let result_square: i32 = script.call("square", (args,)).unwrap();

	assert_eq!(result_triple, 21);
	assert_eq!(result_square, 49);
}

#[test]
fn ctor_error_syntax() {
	let src = "function triple(a) { return 3 *. a; }";
	let script = Script::from_string(src);

	expect_error(script, "Syntax error");
}

#[test]
fn call_error_inexistent_function() {
	// TODO call bad
	let src = "function triple(a) { return 3 * a; }";
	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let args = 7;
	let result: Result<i32, JsError> = script.call("tripel", (args,));

	expect_error(result, "Inexistent function");
}

#[test]
fn call_error_exception() {
	let src = "function triple(a) { throw \"string_error\"; }";
	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let args = 7;
	let result: Result<i32, JsError> = script.call("triple", (args,));

	expect_error(result, "Runtime exception");
}

#[test]
fn call_error_timeout() {
	let timeout = Duration::from_millis(200);
	let expected_stop_time = Duration::from_millis(50);

	let js_code = "function run_forever() { for(;;){} }";
	let mut script = Script::from_string(js_code)
		.expect("Initialization succeeds")
		.with_timeout(timeout);

	let start = Instant::now();
	let result: Result<String, JsError> = script.call("run_forever", ());
	let duration = start.elapsed();

	expect_error(result, "Timed out");
	assert!(
		duration >= timeout,
		"Terminates before the specified timeout (at {}ms)",
		duration.as_millis()
	);
	assert!(
		duration < timeout + expected_stop_time,
		"Took longer than {}ms to terminate (stopped at {}ms)",
		expected_stop_time.as_millis(),
		duration.as_millis()
	);
}

#[test]
fn call_async() {
	let src = r#"
	async function async_func() {
		return new Promise((resolve) => resolve(3));
	}
	"#;

	let mut script = Script::from_string(src).expect("Initialization succeeds");

	let result: i32 = script.call("async_func", ()).unwrap();

	assert_eq!(result, 3);
}

// Regression test for sandbox bypass via `deno_core::FsModuleLoader` (issue #32).
//
// Before the fix, `await import("file:///...", { with: { type: "text" } })` resolved through the default FS loader and
// exfiltrated host file contents to attacker-controlled JS. The default constructors must reject dynamic imports.
#[test]
fn dynamic_import_is_blocked() {
	use std::io::Write;

	struct TempFile(std::path::PathBuf);
	impl Drop for TempFile {
		fn drop(&mut self) {
			let _ = std::fs::remove_file(&self.0);
		}
	}

	// Create a host file with secret contents in the OS temp dir.
	// Wrapped in a Drop guard so the file is removed even if an assertion below panics.
	let mut path = std::env::temp_dir();
	path.push("js-sandbox-test-secret.txt");
	{
		let mut f = std::fs::File::create(&path).expect("create secret file");
		f.write_all(b"TOP_SECRET_VALUE").expect("write secret");
	}
	let _guard = TempFile(path.clone());
	let url = format!("file://{}", path.display());

	let js_code = format!(
		r#"
		async function exfiltrator() {{
			const mod = await import("{url}", {{ with: {{ type: "text" }} }});
			return mod.default;
		}}
		"#
	);

	let mut script = Script::from_string(&js_code).expect("init succeeds");
	let result: Result<String, JsError> = script.call("exfiltrator", ());

	let err = match result {
		Ok(leaked) => {
			panic!("Sandbox bypass: dynamic import returned host file contents: {leaked:?}")
		}
		Err(e) => e,
	};

	let msg = err.to_string();
	assert!(
		!msg.contains("TOP_SECRET_VALUE"),
		"Error message must not leak file contents: {msg}"
	);
}
