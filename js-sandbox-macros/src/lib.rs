// Copyright (c) 2020-2022 js-sandbox contributors. Zlib license.

use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn js_api(_attr: TokenStream, input: TokenStream) -> TokenStream {
	input
}
