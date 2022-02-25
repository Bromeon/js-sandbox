// Copyright (c) 2020-2022 js-sandbox contributors. Zlib license.

use crate::AnyError;
use serde::Serialize;

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
