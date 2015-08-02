use std::borrow::Cow;
// use std::ffi::CString;
use std::mem::transmute;
use std::ops::Deref;
use std::ptr::null_mut;
use std::slice::from_raw_parts;
use libc::c_void;
use cesu8::{to_cesu8, from_cesu8};
use std::ffi::*;

use errors::base::*;
use types::Value;
use duktape_sys::*;

pub mod context;
pub mod callback;

use Context;
use Callback;

/// Convert a duktape-format string into a Rust `String`.
pub unsafe fn from_lstring(data: *const i8, len: duk_size_t) ->
    DuktapeResult<String>
{
    let ptr = data as *const u8;
    let bytes = from_raw_parts(&ptr, len as usize);
    match from_cesu8(bytes) {
        Ok(str) => Ok(str.into_owned()),
        Err(_) => Err(DuktapeError::from_str("can't convert string to UTF-8"))
    }
}


#[cfg(test)]
#[allow(missing_docs)]
mod test {
    use errors::*;
    use types::*;
    use super::*;

    pub fn rust_add(_ctx: &mut Context, args: &[Value<'static>]) ->
        DuktapeResult<Value<'static>>
    {
        let mut sum = 0.0;
        for arg in args.iter() {
            // TODO: Type checking.
            if let &Value::Number(n) = arg {
                sum += n;
            }
        }
        Ok(Value::Number(sum))
    }

    macro_rules! rust_callback {
        ($name:ident, $retval:expr) => {
            pub fn $name(_ctx: &mut Context, _args: &[Value<'static>]) ->
                DuktapeResult<Value<'static>>
            {
                $retval
            }
        }
    }

    rust_callback!{rust_return_undefined, Ok(Value::Undefined)}
    rust_callback!{rust_return_simple_error,
                   Err(DuktapeError::from_code(ErrorCode::Type))}
    rust_callback!{rust_return_custom_error,
                   Err(DuktapeError::from_str("custom error"))}
}

#[test]
fn test_callbacks() {
    let mut ctx = Context::new().unwrap();

    // An ordinary function, with arguments and a useful return value.
    ctx.register("add", test::rust_add, Some(2));
    assert_eq!(Value::Number(5.0), ctx.eval("add(2.0, 3.0)").unwrap());

    // A funtion which returns `undefined` (the same as having no return
    // value).
    ctx.register("ret_undefined", test::rust_return_undefined, Some(0));
    assert_eq!(Value::Undefined, ctx.eval("ret_undefined()").unwrap());

    // A function which returns a numeric error code (special-cased in
    // duktape).
    ctx.register("simple_error", test::rust_return_simple_error, Some(0));
    assert!(ctx.eval("simple_error()").is_err());

    // A function which returns a custom error with a string.
    ctx.register("custom_error", test::rust_return_custom_error, Some(0));
    let res = ctx.eval("custom_error()");
    assert!(res.is_err());
}
