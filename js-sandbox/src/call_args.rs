// Copyright (c) 2020-2022 js-sandbox contributors. Zlib license.

use crate::AnyError;
use serde::Serialize;

/// Trait that is implemented for types that can be passed as argument to `Script::call()`.
///
/// This is currently only implemented for tuples of size 0..=5, i.e. JS functions with 0 to 5 arguments.
/// Use structs or arrays inside a one-element tuple if you need more flexibility.
pub trait CallArgs {
	fn into_arg_string(self) -> Result<String, AnyError>;
}

impl CallArgs for () {
	fn into_arg_string(self) -> Result<String, AnyError> {
		Ok(String::new())
	}
}

macro_rules! impl_call_args {
	($($param:ident),+) => {
		#[allow(non_snake_case)] // use generic params as variable names
		impl<$($param),+> CallArgs for ($($param),+,)
			where $($param : Serialize),+
		{
			fn into_arg_string(self) -> Result<String, AnyError> {
				let ($($param),+,) = self;
				let args = [
					$(
						serde_json::to_value($param)?.to_string()
					),+
				];

				Ok(args.join(","))
			}
		}
	}
}

impl_call_args!(P0);
impl_call_args!(P0, P1);
impl_call_args!(P0, P1, P2);
impl_call_args!(P0, P1, P2, P3);
impl_call_args!(P0, P1, P2, P3, P4);
