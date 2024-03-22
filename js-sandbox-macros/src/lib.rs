// Copyright (c) 2020-2023 js-sandbox contributors. Zlib license.

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{quote, ToTokens};
use syn::spanned::Spanned as _;

#[proc_macro_attribute]
pub fn js_api(_attr: TokenStream, input: TokenStream) -> TokenStream {
	let item = syn::parse_macro_input!(input as syn::ItemTrait);

	let stream2 = match generate_api(item) {
		Ok(stream) => stream,
		Err(err) => err.to_compile_error(),
	};

	TokenStream::from(stream2)
}

fn generate_api(item: syn::ItemTrait) -> syn::Result<TokenStream2> {
	let name = &item.ident;
	let struct_ = generate_struct(&item)?;
	let methods = generate_impl_methods(&item)?;
	let marker_impl = generate_marker_trait_impl(&item)?;

	Ok(quote! {
		#struct_
		impl<'a> #name<'a> {
			#methods
		}
		#marker_impl
	})
}

fn generate_struct(item: &syn::ItemTrait) -> syn::Result<TokenStream2> {
	let name = &item.ident;
	let visibility = &item.vis;

	Ok(quote! {
		#visibility struct #name<'a> {
			script: &'a mut js_sandbox_ios::Script,
		}
	})
}

fn generate_marker_trait_impl(item: &syn::ItemTrait) -> syn::Result<TokenStream2> {
	let name = &item.ident;
	let visibility = &item.vis;

	Ok(quote! {
		impl<'a> js_sandbox_ios::JsApi<'a> for #name<'a> {
			#visibility fn from_script(script: &'a mut js_sandbox_ios::Script) -> Self {
				Self { script }
			}
		}
	})
}

macro_rules! syntax_error {
	($err:expr, $($fmt:tt)*) => (
		{ return Err(syn::Error::new($err.span(), format!($($fmt)*))); }
	)
}

enum ReturnType {
	Unit,
	Direct(syn::Type),
	ResultWrap(syn::Type),
}

fn generate_impl_methods(item: &syn::ItemTrait) -> syn::Result<TokenStream2> {
	let mut result = TokenStream2::new();
	for item in item.items.iter() {
		let method = match item {
			syn::TraitItem::Fn(f) => f,
			other => syntax_error!(other, "only methods are allowed"),
		};
		if let Some(tok) = &method.sig.constness {
			syntax_error!(tok, "const functions are not supported");
		}
		if let Some(tok) = &method.sig.asyncness {
			syntax_error!(tok, "async functions are not supported");
		}
		if let Some(tok) = &method.default {
			syntax_error!(tok, "cannot specify an implementation of methods");
		}
		if let Some(rcv) = method.sig.receiver() {
			if rcv.mutability.is_none() {
				syntax_error!(
					rcv,
					"receiver must be `&mut self`; values and shared references are not supported"
				);
			}
		} else {
			syntax_error!(
				method.sig.ident,
				"receiver must be `&mut self`; associated methods are not supported"
			);
		}

		let mut args = Vec::new();
		for arg in method.sig.inputs.iter() {
			let arg = match arg {
				syn::FnArg::Receiver(_) => continue,
				syn::FnArg::Typed(arg) => arg,
			};
			let ident = match &*arg.pat {
				syn::Pat::Ident(i) => i,
				other => syntax_error!(other, "parameter must be a bare identifier"),
			};
			if let Some(tok) = &ident.by_ref {
				syntax_error!(tok, "parameter must be a value; by-reference unsupported");
			}
			if let Some(tok) = &ident.mutability {
				syntax_error!(tok, "parameter must not be mutable");
			}
			if let Some((_, tok)) = &ident.subpat {
				syntax_error!(tok, "parameter cannot have destructured bindings");
			}

			let ident = &ident.ident;
			args.push(ident);
		}

		let sig = &method.sig;
		let attrs = &method.attrs;
		let fn_name = quote_token(&method.sig.ident);

		let return_type: TokenStream2;
		let transform: TokenStream2;
		match parse_return_type(&method.sig.output)? {
			ReturnType::Direct(ty) => {
				return_type = ty.to_token_stream();
				let ty_str = quote_token(&ty);
				transform = quote! {
					result.expect(concat!("cannot convert to type `", #ty_str, "`"))
				}
			}
			ReturnType::ResultWrap(ty) => {
				return_type = ty.to_token_stream();
				transform = quote! { result };
			}
			ReturnType::Unit => {
				return_type = quote! { () };
				transform = quote! {
					result.expect("JS function call failed");
				}
			}
		};

		result.extend(quote! {
			#(#attrs)*
			#sig {
				let args = (
					#(#args,),*
				);

				let result: js_sandbox_ios::JsResult<#return_type> = self.script.call(#fn_name, args);
				#transform
			}
		});
	}

	Ok(result)
}

fn parse_return_type(tok: &syn::ReturnType) -> syn::Result<ReturnType> {
	match tok {
		syn::ReturnType::Default => {
			// no explicit return type
			Ok(ReturnType::Unit)
		}
		syn::ReturnType::Type(_, ty) => {
			if let syn::Type::Path(path) = ty.as_ref() {
				let seg = path.path.segments.first();

				if seg.is_none() {
					syntax_error!(tok, "unsupported return type; expected T or JsResult<T>, where T: Deserializable")
				}

				let seg = seg.unwrap();

				if seg.ident == "JsResult" || seg.ident == "js_sandbox_ios::JsResult" {
					// -> JsResult<T>
					match &seg.arguments {
						syn::PathArguments::None => {}
						syn::PathArguments::AngleBracketed(ret) => {
							if let Some(syn::GenericArgument::Type(ty)) = ret.args.first() {
								return Ok(ReturnType::ResultWrap((*ty).clone()));
							}
						}
						syn::PathArguments::Parenthesized(_) => {}
					}
				} else {
					// -> T
					return Ok(ReturnType::Direct((**ty).clone()));
				}
			}

			syntax_error!(
				tok,
				"unsupported return type; expected T or JsResult<T>, where T: Deserializable"
			)
		}
	}
}

fn quote_token(token: &dyn quote::ToTokens) -> syn::Lit {
	syn::Lit::Str(syn::LitStr::new(
		&token.to_token_stream().to_string(),
		token.span(),
	))
}
