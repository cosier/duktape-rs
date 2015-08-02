
use errors::base::DuktapeResult;
use types::Value;
use duktape_sys::*;

use Context;

/// A Rust callback which can be invoked from JavaScript.
pub type Callback = fn (&mut Context, &[Value<'static>]) ->
    DuktapeResult<Value<'static>>;
