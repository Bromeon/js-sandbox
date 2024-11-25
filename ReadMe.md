<!-- DO NOT EDIT -- ReadMe is generated from lib.rs -->

# js-sandbox

[<img alt="crates.io" src="https://img.shields.io/crates/v/js-sandbox?logo=rust&color=A6854D" />](https://crates.io/crates/js-sandbox)
[<img alt="docs.rs" src="https://img.shields.io/badge/docs.rs-js--sandbox-4D8AA6?&logo=data:image/svg+xml;base64,PHN2ZyByb2xlPSJpbWciIHhtbG5zPSJodHRwOi8vd3d3LnczLm9yZy8yMDAwL3N2ZyIgdmlld0JveD0iMCAwIDUxMiA1MTIiPjxwYXRoIGZpbGw9IiNmNWY1ZjUiIGQ9Ik00ODguNiAyNTAuMkwzOTIgMjE0VjEwNS41YzAtMTUtOS4zLTI4LjQtMjMuNC0zMy43bC0xMDAtMzcuNWMtOC4xLTMuMS0xNy4xLTMuMS0yNS4zIDBsLTEwMCAzNy41Yy0xNC4xIDUuMy0yMy40IDE4LjctMjMuNCAzMy43VjIxNGwtOTYuNiAzNi4yQzkuMyAyNTUuNSAwIDI2OC45IDAgMjgzLjlWMzk0YzAgMTMuNiA3LjcgMjYuMSAxOS45IDMyLjJsMTAwIDUwYzEwLjEgNS4xIDIyLjEgNS4xIDMyLjIgMGwxMDMuOS01MiAxMDMuOSA1MmMxMC4xIDUuMSAyMi4xIDUuMSAzMi4yIDBsMTAwLTUwYzEyLjItNi4xIDE5LjktMTguNiAxOS45LTMyLjJWMjgzLjljMC0xNS05LjMtMjguNC0yMy40LTMzLjd6TTM1OCAyMTQuOGwtODUgMzEuOXYtNjguMmw4NS0zN3Y3My4zek0xNTQgMTA0LjFsMTAyLTM4LjIgMTAyIDM4LjJ2LjZsLTEwMiA0MS40LTEwMi00MS40di0uNnptODQgMjkxLjFsLTg1IDQyLjV2LTc5LjFsODUtMzguOHY3NS40em0wLTExMmwtMTAyIDQxLjQtMTAyLTQxLjR2LS42bDEwMi0zOC4yIDEwMiAzOC4ydi42em0yNDAgMTEybC04NSA0Mi41di03OS4xbDg1LTM4Ljh2NzUuNHptMC0xMTJsLTEwMiA0MS40LTEwMi00MS40di0uNmwxMDItMzguMiAxMDIgMzguMnYuNnoiPjwvcGF0aD48L3N2Zz4K" />](https://docs.rs/js-sandbox)

`js-sandbox` is a Rust library for executing JavaScript code from Rust in a secure sandbox. It is based on the [Deno] project and uses [serde_json]
for serialization.

This library's primary focus is **embedding JS as a scripting language into Rust**. It does not provide all possible integrations between the two
languages, and is not tailored to JS's biggest domain as a client/server side language of the web.

Instead, `js-sandbox` focuses on calling standalone JS code from Rust, and tries to remain as simple as possible in doing so.
The typical use case is a core Rust application that integrates with scripts from external users, for example a plugin system or a game that runs
external mods.

> [!TIP]
> If you are looking for a more feature-rich integration, check out [rustyscript](https://github.com/rscarson/rustyscript)!

## Examples

### Print from JavaScript

The _Hello World_ example -- print something using JavaScript -- is one line, as it should be:
```rust
fn main() {
    js_sandbox::eval_json("console.log('Hello Rust from JS')").expect("JS runs");
}
```

### Call a JS function

A very basic application calls a JavaScript function `sub()` from Rust. It passes an argument and accepts a return value, both serialized via JSON:

```rust
use js_sandbox::{Script, AnyError};

fn main() -> Result<(), AnyError> {
    let js_code = "function sub(a, b) { return a - b; }";
    let mut script = Script::from_string(js_code)?;

    let result: i32 = script.call("sub", (7, 5))?;

    assert_eq!(result, 2);
    Ok(())
}
```

An example that serializes a JSON object (Rust -> JS) and formats a string (JS -> Rust):

```rust
use js_sandbox::{Script, AnyError};
use serde::Serialize;

#[derive(Serialize)]
struct Person {
    name: String,
    age: u8,
}

fn main() -> Result<(), AnyError> {
    let src = r#"
        function toString(person) {
            return "A person named " + person.name + " of age " + person.age;
        }"#;

    let mut script = Script::from_string(src)?;

    let person = Person { name: "Roger".to_string(), age: 42 };
    let result: String = script.call("toString", (person,))?;

    assert_eq!(result, "A person named Roger of age 42");
    Ok(())
}
```

### Load JS file

JavaScript files can be loaded from any `Path` at runtime (e.g. 3rd party mods).

If you want to statically embed UTF-8 encoded files in the Rust binary, you can alternatively use the
[`std::include_str`](https://doc.rust-lang.org/std/macro.include_str.html) macro.

```rust
use js_sandbox::Script;

fn main() {
    // (1) at runtime:
    let mut script = Script::from_file("script.js").expect("load + init succeeds");

    // (2) at compile time:
    let code: &'static str = include_str!("script.js");
    let mut script = Script::from_string(code).expect("init succeeds");

    // use script as usual
}
```

### Maintain state in JavaScript

It is possible to initialize a stateful JS script, and then use functions to modify that state over time.
This example appends a string in two calls, and then gets the result in a third call:

```rust
use js_sandbox::{Script, AnyError};

fn main() -> Result<(), AnyError> {
    let src = r#"
        var total = '';
        function append(str) { total += str; }
        function get()       { return total; }"#;

    let mut script = Script::from_string(src)?;

    let _: () = script.call("append", ("hello",))?;
    let _: () = script.call("append", (" world",))?;
    let result: String = script.call("get", ())?;

    assert_eq!(result, "hello world");
    Ok(())
}
```

### Call a script with timeout

The JS code may contain long- or forever-running loops that block Rust code. It is possible to set
a timeout, after which JavaScript execution is aborted.

```rust
use js_sandbox::{Script, JsError};

fn main() -> Result<(), JsError> {
    use std::time::Duration;
    let js_code = "function run_forever() { for(;;) {} }";
    let mut script = Script::from_string(js_code)?
        .with_timeout(Duration::from_millis(1000));

    let result: Result<String, JsError> = script.call("run_forever", ());

    assert_eq!(
        result.unwrap_err().to_string(),
        "Uncaught Error: execution terminated".to_string()
    );

    Ok(())
}
```

[Deno]: https://deno.land
[serde_json]: https://docs.serde.rs/serde_json
