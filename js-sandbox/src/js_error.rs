use std::{
	error::Error,
	fmt::{self, Display},
};

use crate::AnyError;

/// Represents an error ocurring during script execution
#[derive(Debug)]
pub enum JsError {
	Json(serde_json::Error),
	Runtime(AnyError),
}

impl Error for JsError {}

impl Display for JsError {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		match self {
			JsError::Json(e) => write!(f, "{}", e),
			JsError::Runtime(e) => write!(f, "{}", e),
		}
	}
}

impl From<AnyError> for JsError {
	fn from(e: AnyError) -> JsError {
		JsError::Runtime(e)
	}
}

impl From<serde_json::Error> for JsError {
	fn from(e: serde_json::Error) -> JsError {
		JsError::Json(e)
	}
}
