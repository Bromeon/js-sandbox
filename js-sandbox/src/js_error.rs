use std::{
	error::Error,
	fmt::{self, Display},
};

/// Error occurring during script execution.
///
/// Variants partition errors by *who is responsible* for fixing them, so callers
/// can branch on remediation rather than on opaque error strings.
#[derive(Debug)]
pub enum JsError {
	/// The user-supplied JavaScript misbehaved: thrown exception, syntax error,
	/// or execution terminated by a timeout. Fix the script.
	Runtime(Box<deno_core::error::JsError>),

	/// A value could not be (de-)serialized across the Rust↔JS boundary, in
	/// either direction. Fix the Rust types or the JS contract — they disagree.
	Conversion(Box<dyn Error + Send + Sync + 'static>),

	/// Internal failure not attributable to the script: file I/O while loading
	/// a script, event-loop / module-loader errors, etc. Usually propagated.
	Logic(Box<dyn Error + Send + Sync + 'static>),
}

impl Error for JsError {
	fn source(&self) -> Option<&(dyn Error + 'static)> {
		match self {
			JsError::Runtime(e) => Some(e),
			JsError::Conversion(e) | JsError::Logic(e) => Some(e.as_ref()),
		}
	}
}

impl Display for JsError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			JsError::Runtime(e) => write!(f, "{e}"),
			JsError::Conversion(e) => write!(f, "{e}"),
			JsError::Logic(e) => write!(f, "{e}"),
		}
	}
}

impl From<serde_json::Error> for JsError {
	fn from(e: serde_json::Error) -> JsError {
		JsError::Conversion(Box::new(e))
	}
}

impl From<deno_core::serde_v8::Error> for JsError {
	fn from(e: deno_core::serde_v8::Error) -> JsError {
		JsError::Conversion(Box::new(e))
	}
}

impl From<Box<deno_core::error::JsError>> for JsError {
	fn from(e: Box<deno_core::error::JsError>) -> JsError {
		JsError::Runtime(e)
	}
}

impl From<deno_core::error::CoreError> for JsError {
	fn from(e: deno_core::error::CoreError) -> JsError {
		// CoreError straddles categories: peel off the Js variant so a thrown
		// or terminated script surfaces as Runtime; the rest is infrastructure.
		match *e.0 {
			deno_core::error::CoreErrorKind::Js(js) => JsError::Runtime(js),
			other => JsError::Logic(Box::new(other)),
		}
	}
}

impl From<std::io::Error> for JsError {
	fn from(e: std::io::Error) -> JsError {
		JsError::Logic(Box::new(e))
	}
}
