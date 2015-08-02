//! Rust interface to [Duktape][] JavaScript interpreter.  This is still
//! a work in progress!
//!
//! [Source code](https://github.com/emk/duktape-rs).
//!
//! ```
//! use duktape::{Context,Value,DuktapeResult};
//!
//! fn add_example() -> DuktapeResult<Value<'static>> {
//!     // Create a new JavaScript interpreter.  This will be automatically
//!     // cleaned up when `ctx` goes out of scope.
//!     let mut ctx = try!(Context::new());
//!
//!     // Load some code from a string.
//!     try!(ctx.eval("function add(x, y) { return x+y; }"));
//!
//!     // Call the function we defined.
//!     ctx.call("add", &[&2.0f64, &1.0f64])
//! }
//!
//! assert_eq!(Ok(Value::Number(3.0)), add_example());
//! ```
//!
//! We also have preliminary support for defining JavaScript functions
//! using Rust, but it's still too ugly to show off.
//!
//! [Duktape]: http://duktape.org/

#![feature(std_misc)]
#![feature(collections)]
#![feature(core)]
#![feature(libc)]
#![feature(convert)]

#![allow(missing_docs)]
#![allow(unused_features)]
#![allow(unused_attributes)]
#![allow(unused_imports)]
#![allow(dead_code)]

#[macro_use] extern crate log;
extern crate rustc_serialize;
extern crate libc;
extern crate cesu8;
#[macro_use] extern crate abort_on_panic;
extern crate duktape_sys;

// use errors::{ErrorCode, DuktapeError, DuktapeResult};
// use types::Value;
// use context::{Context, Callback};
use duktape_sys::*;

#[macro_use]
mod macros;

pub use contexts::callback::Callback;
pub use contexts::context::Context;

mod contexts;
mod io;
mod errors;
mod types;

