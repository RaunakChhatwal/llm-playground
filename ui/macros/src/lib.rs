use proc_macro::TokenStream;
use quote::quote;
use syn::{ItemFn, FnArg, parse_macro_input, Pat};

#[proc_macro_attribute]
pub fn command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_function = parse_macro_input!(item as ItemFn);
    let function_name = input_function.sig.ident;
    let visibility = input_function.vis;
    let arguments = input_function.sig.inputs;
    let return_type = input_function.sig.output;

    let syn::ReturnType::Type(_, return_type) = return_type else {
        panic!("Return type must be specified.");
    };

    let insert_statements = arguments
        .iter()
        .map(|argument| {
            let FnArg::Typed(argument) = argument else {
                panic!("Command can't be a method.");
            };

            let Pat::Ident(argument) = *argument.pat.clone() else {
                panic!("Parameters must be an identifier rather than pattern.");
            };

            let argument = argument.ident;
            quote! {
                arguments_map.insert(stringify!(#argument).to_string(), serde_json::to_value(#argument)
                    .context(format!("Error serializing arguments to {function_name}"))?);
            }
        })
        .collect::<Vec<_>>();

    return TokenStream::from(quote! {
        #visibility async fn #function_name(#arguments) -> #return_type {
            use anyhow::{anyhow, Context};
            use gloo_utils::format::JsValueSerdeExt;
            use wasm_bindgen::{JsValue, prelude::*};

            #[wasm_bindgen]
            extern "C" {
                #[wasm_bindgen(catch, js_namespace = ["window", "__TAURI__", "tauri"])]
                async fn invoke(cmd: &str, args: JsValue) -> Result<JsValue, JsValue>;
            }

            let function_name = stringify!(#function_name);

            let mut arguments_map = serde_json::Map::new();
            #(#insert_statements)*
            let arguments_jsvalue = JsValue::from_serde(&serde_json::Value::Object(arguments_map))
                .context(format!("Error serializing arguments to {function_name}"))?;

            match invoke(function_name, arguments_jsvalue).await {
                Ok(result) => Ok(JsValue::into_serde(&result)?),
                Err(error) => Err(JsValue::into_serde::<serde_error::Error>(&error).ok()
                    .map(anyhow::Error::from)
                    .unwrap_or(anyhow!("Error invoking {function_name}")))
            }
        }
    });
}