// Copyright (c) 2020-2021 Jan Haller. zlib/libpng license.

use std::time::Instant;

use serde::{Deserialize, Serialize};

use js_sandbox::{AnyError, Script};
use util::expect_error;

mod util;

#[derive(Serialize, Debug, PartialEq)]
struct JsArgs {
	text: String,
	num: i32,
}

#[derive(Deserialize, Debug, PartialEq)]
struct JsResult {
	new_text: String,
	new_num: i32,
}

#[derive(Serialize, PartialEq)]
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

	let mut script = Script::from_string(src)
		.expect("Initialization succeeds");

	let args = JsArgs { text: "hi".to_string(), num: 4 };
	let exp_result = JsResult { new_text: "hi.".to_string(), new_num: 12 };

	let result: JsResult = script.call("extract", &args, None).unwrap();
	assert_eq!(result, exp_result);
}

#[test]
fn call_string() {
	let src = r#"
	function toString(person) {
		return "A person named " + person.name + " with age " + person.age;
	}"#;

	let mut script = Script::from_string(src)
		.expect("Initialization succeeds");

	let person = Person { name: "Roger".to_string(), age: 42 };
	let result: String = script.call("toString", &person, None).unwrap();

	assert_eq!(result, "A person named Roger with age 42");
}

#[test]
fn call_minimal() -> Result<(), AnyError> {
	let js_code = "function triple(a) { return 3 * a; }";
	let mut script = Script::from_string(js_code)?;

	let args = 7;
	let result: i32 = script.call("triple", &args, None)?;

	assert_eq!(result, 21);
	Ok(())
}

#[test]
fn call_void() -> Result<(), AnyError> {
	let js_code = "function print(expr) { console.log(expr); }";
	let mut script = Script::from_string(js_code)?;

	let args = "some text";
	let _result: () = script.call("print", &args, None)?;

	Ok(())
}

#[test]
fn call_from_file() {
	let mut script = Script::from_file("tests/hello.js")
		.expect("File can be loaded");

	let args = JsArgs { text: "hi".to_string(), num: 4 };
	let exp_result = JsResult { new_text: "hi.".to_string(), new_num: 12 };

	let result: JsResult = script.call("extract", &args, None).unwrap();
	assert_eq!(result, exp_result);
}

#[test]
fn call_local_state() {
	let src = "var i = 0;
	function inc() { return ++i; }";
	let mut script = Script::from_string(src)
		.expect("Initialization succeeds");

	let args = ();

	let result: i32 = script.call("inc", &args, None).unwrap();
	assert_eq!(result, 1);

	let result: i32 = script.call("inc", &args, None).unwrap();
	assert_eq!(result, 2);
}

#[test]
fn call_repeated() {
	let src = "
		function triple(a) { return 3 * a; }
		function square(a) { return a * a; }";

	let mut script = Script::from_string(src)
		.expect("Initialization succeeds");

	let args = 7;

	let result_triple: i32 = script.call("triple", &args, None).unwrap();
	let result_square: i32 = script.call("square", &args, None).unwrap();

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
	let mut script = Script::from_string(src)
		.expect("Initialization succeeds");

	let args = 7;
	let result: Result<i32, AnyError> = script.call("tripel", &args, None);

	expect_error(result, "Inexistent function");
}

#[test]
fn call_error_exception() {
	let src = "function triple(a) { throw \"string_error\"; }";
	let mut script = Script::from_string(src)
		.expect("Initialization succeeds");

	let args = 7;
	let result: Result<i32, AnyError> = script.call("triple", &args, None);

	expect_error(result, "Runtime exception");
}

#[test]
fn call_error_timeout() {
	let timeout = 200;
	let expected_stop_time = 50;

	let js_code = "function run_forever() { for(;;){} }";
	let mut script = Script::from_string(js_code)
		.expect("Initialization succeeds");

	let start = Instant::now();
	let result: Result<String, AnyError> = script.call("run_forever", &(), Some(timeout));
	let duration = start.elapsed().as_millis() as u64;

	expect_error(result, "Timed out");
	assert!(duration >= timeout, "Terminates before the specified timeout (at {}ms)", duration);
	assert!(duration < timeout + expected_stop_time, "Took longer than {}ms to terminate (stopped at {}ms)", expected_stop_time, duration);
}