#![feature(type_alias_impl_trait)]
#![feature(fn_traits)]

mod data;
pub use data::Data;

mod factory;

pub mod route;

use jsonrpc_lite::Error as JsonRpcError;

fn server_route_error() -> JsonRpcError {
    JsonRpcError {
        code: -32500,
        message: "Server Internal Route error".to_string(),
        data: None,
    }
}