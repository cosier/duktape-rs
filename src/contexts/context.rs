use std::borrow::Cow;
use std::ffi::CString;
use std::mem::transmute;
use std::ops::Deref;
use std::ptr::null_mut;
use std::slice::from_raw_parts;
use std::string::String;
use std::ffi::CStr;
use std::str;
use libc;
use libc::c_void;
use cesu8::{to_cesu8, from_cesu8};

use types::Value;

use duktape_sys::*;
use errors::base::*;

use contexts::from_lstring;
use Callback;
use io::encoder::{Encoder, DuktapeEncodable};


/// A duktape interpreter context.  An individual context is not
/// re-entrant: You may only access it from one thread at a time.
pub struct Context {
    ptr: *mut duk_context,
    owned: bool
}

impl Context {
    /// Create a new duktape context.
    pub fn new() -> DuktapeResult<Context> {
        let ptr = unsafe {
            duk_create_heap(None, None, None, null_mut(), None)
        };
        if ptr.is_null() {
            Err(DuktapeError::from_str("Could not create heap"))
        } else {
            Ok(Context{ptr: ptr, owned: true})
        }
    }

    /// Create a new duktape context by wrapping an existing mutable
    /// pointer.  This is potentially unsafe, because it allows you to
    /// create two Rust objects pointing to the same duktape interpreter!
    /// So if you create a Context using this API
    pub unsafe fn from_borrowed_mut_ptr(ptr: *mut duk_context) -> Context {
        Context{ptr: ptr, owned: false}
    }

    /// Get the underlying context pointer.  You generally don't need this
    /// unless you're implementing low-level add-ons to this library.
    pub unsafe fn as_mut_ptr(&mut self) -> *mut duk_context { self.ptr }

    /// Debugging: Dump the interpreter context.
    #[allow(dead_code)]
    fn dump_context(&mut self) -> String {
        unsafe {
            duk_push_context_dump(self.ptr);
            let mut len: duk_size_t = 0;
            let str = duk_safe_to_lstring(self.ptr, -1, &mut len);
            let result = from_lstring(str, len)
                .unwrap_or_else(|_| "Couldn't dump context".to_string());
            duk_pop(self.ptr);
            result
        }
    }

    /// Get the specified value from our context, and convert it to a Rust
    /// type.  This is a low-level, unsafe function, and you won't normally
    /// need to call it.
    unsafe fn get(&mut self, idx: duk_idx_t) -> DuktapeResult<Value<'static>> {
        match duk_get_type(self.ptr, idx) {
            DUK_TYPE_UNDEFINED => Ok(Value::Undefined),
            DUK_TYPE_NULL => Ok(Value::Null),
            DUK_TYPE_BOOLEAN => {
                let val = duk_get_boolean(self.ptr, idx);
                Ok(Value::Bool(val != 0))
            }
            DUK_TYPE_NUMBER => {
                Ok(Value::Number(duk_get_number(self.ptr, idx)))
            }
            DUK_TYPE_STRING => {
                let mut len: duk_size_t = 0;
                let str = duk_get_lstring(self.ptr, idx, &mut len);
                Ok(Value::String(Cow::Owned(try!(from_lstring(str, len)))))
            }
            _ => panic!("Cannot convert duktape data type")
        }
    }

    /// Push a value to the call stack.
    pub unsafe fn push_old(&mut self, val: &Value) {
        match val {
            &Value::Undefined => duk_push_undefined(self.ptr),
            &Value::Null => duk_push_null(self.ptr),
            &Value::Bool(v) => duk_push_boolean(self.ptr, if v { 1 } else { 0 }),
            &Value::Number(v) => duk_push_number(self.ptr, v),
            &Value::String(ref v) => {
                let encoded = to_cesu8(v.deref());
                let buf = encoded.deref();
                duk_push_lstring(self.ptr, buf.as_ptr() as *const i8,
                                 buf.len() as duk_size_t);
            }
        }
    }

    /// Push an encodable value onto the call stack.  We can push any data
    /// type that implements Encodable.
    pub unsafe fn push<T: DuktapeEncodable>(&mut self, object: &T) {
        let mut encoder = Encoder::new(self.ptr);
        object.duktape_encode(&mut encoder).unwrap();
    }

    /// Interpret the value on the top of the stack as either a return
    /// value or an error, depending on the value of `status`.
    unsafe fn get_result(&mut self, status: duk_int_t) ->
        DuktapeResult<Value<'static>>
    {
        if status == DUK_EXEC_SUCCESS {
            self.get(-1)
        } else {
            let mut len: duk_size_t = 0;
            let str = duk_safe_to_lstring(self.ptr, -1, &mut len);
            let msg = try!(from_lstring(str, len));
            Err(DuktapeError::from_str(&msg))
        }
    }

    /// Given the status code returned by a duktape exec function, pop
    /// either a value or an error from the stack, convert it, and return
    /// it.
    pub unsafe fn pop_result(&mut self, status: duk_int_t) ->
        DuktapeResult<Value<'static>>
    {
        let result = self.get_result(status);
        duk_pop(self.ptr);
        result
    }

    /// Evaluate JavaScript source code and return the result.
    pub fn eval(&mut self, code: &str) -> DuktapeResult<Value<'static>> {
        self.eval_from("<eval>", code)
    }

    /// Evaluate JavaScript source code and return the result.  The
    /// `filename` parameter will be used in any error messages.
    pub fn eval_from(&mut self, filename: &str, code: &str) ->
        DuktapeResult<Value<'static>>
    {
        unsafe {
            assert_stack_height_unchanged!(self, {
                // Push our filename parameter and evaluate our code.
                duk_push_lstring(self.ptr, filename.as_ptr() as *const i8,
                                 filename.len() as duk_size_t);
                let status = duk_eval_raw(self.ptr, code.as_ptr() as *const i8,
                                          code.len() as duk_size_t,
                                          DUK_COMPILE_EVAL |
                                          DUK_COMPILE_NOSOURCE |
                                          DUK_COMPILE_SAFE);
                self.pop_result(status)
            })
        }
    }

    /// Call the global JavaScript function named `fn_name` with `args`, and
    /// return the result.
    pub fn call(&mut self, fn_name: &str, args: &[&DuktapeEncodable]) ->
        DuktapeResult<Value<'static>>
    {
        unsafe {
            assert_stack_height_unchanged!(self, {
                duk_push_global_object(self.ptr);
                let c_str = CString::new(fn_name).unwrap();
                duk_get_prop_string(self.ptr, -1, c_str.as_ptr());
                {
                    let mut encoder = Encoder::new(self.ptr);
                    for arg in args.iter() {
                        (*arg).duktape_encode(&mut encoder).unwrap();
                    }
                }
                let status = duk_pcall(self.ptr, args.len() as i32);
                let result = self.pop_result(status);
                duk_pop(self.ptr); // Remove global object.
                result
            })
        }
    }

    /// Register a Rust callback as a global JavaScript function.
    pub fn register(&mut self, fn_name: &str, f: Callback,
                    arg_count: Option<u16>) {
        let c_arg_count =
            arg_count.map(|n| n as duk_int_t).unwrap_or(DUK_VARARGS);
        unsafe {
            assert_stack_height_unchanged!(self, {
                // Push our global context and a pointer to our standard
                // wrapper function.
                duk_push_global_object(self.ptr);
                duk_push_c_function(self.ptr,
                                    Some(rust_duk_callback),
                                    c_arg_count);

                // Store `f` as a hidden property in our function.
                duk_push_pointer(self.ptr, f as *mut c_void);
                duk_put_prop_string(self.ptr, -2, RUST_FN_PROP.as_ptr());

                // Store our function in a global property.
                let c_str = CString::new(fn_name).unwrap();
                duk_put_prop_string(self.ptr, -2, c_str.as_ptr());
                duk_pop(self.ptr);
            })
        }
    }
}

impl Drop for Context {
  fn drop(&mut self) {
      if self.owned {
          unsafe { duk_destroy_heap(self.ptr); }
      }
  }
}

/// A "internal" property key used for storing Rust function pointers, which
/// can't be accessed from JavaScript without a lot of trickery.
const RUST_FN_PROP: [i8; 5] = [-1, 'r' as i8, 'f' as i8, 'n' as i8, 0];

/// Our generic callback function.
unsafe extern "C" fn rust_duk_callback(ctx: *mut duk_context) -> duk_ret_t {
    // ERROR-HANDLING NOTE: Try to avoid any Rust panics or duktape unwinds
    // inside this function.  They sort-of work--at least well enough to
    // debug this crate--but they probably corrupt at least one of the two
    // heaps.

    // Here, we create a mutable Context pointing into an existing duktape
    // heap.  But this is theoretically safe, because the only way to
    // invoke JavaScript code is to use a mutable context while calling
    // into C.  So this is really an indirect mutable borrow.
    assert!(ctx != null_mut());
    let mut ctx = Context::from_borrowed_mut_ptr(ctx);
    //println!("In callback: {}", ctx.dump_context());

    // Recover our Rust function pointer.
    let f: Callback = assert_stack_height_unchanged!(ctx, {
        duk_push_current_function(ctx.ptr);
        duk_get_prop_string(ctx.ptr, -1, RUST_FN_PROP.as_ptr());
        let p = duk_get_pointer(ctx.ptr, -1);
        duk_pop_n(ctx.ptr, 2);
        assert!(p != null_mut());
        transmute(p)
    });

    // Coerce our arguments to Rust values.
    let arg_count = duk_get_top(ctx.ptr) as usize;
    let mut args = Vec::with_capacity(arg_count);
    for i in 0..arg_count {
        match ctx.get(i as duk_idx_t) {
            Ok(arg) => args.push(arg),
            // Can't convert argument to Rust.
            // TODO: Need testcase.
            Err(_) => return DUK_RET_TYPE_ERROR
        }
    }
    //println!("args: {}", args);

    // Call our function.
    let result =
        abort_on_panic!("unexpected panic in code called from JavaScript", {
            f(&mut ctx, &args)
        });

    // Return our result.
    match result {
        // No return value.
        Ok(Value::Undefined) => { 0 }
        // A single return value.
        Ok(ref val) => { ctx.push_old(val); 1 }
        Err(ref err) => {
            let code = err_code(err) as duk_int_t;
            match err_message(err) {
                // An error with an actual error message.
                &Some(ref _msg) => {
                    // The following would more-or-less work, but it
                    // performs a non-local exit from a Rust function using
                    // C APIs, which is a Bad Idea.
                    //to_cesu8(&msg[]).with_c_str(|c_str| {
                    //    duk_push_error_object_string(ctx.ptr, code,
                    //                                 file!().as_ptr()
                    //                                     as *const i8,
                    //                                 line!() as i32,
                    //                                 c_str as *const i8);
                    //});
                    //duk_throw(ctx.ptr);
                    //-1
                    DUK_RET_ERROR
                }
                // A generic error using one of the standard codes.
                &None => { -code }
            }
        }
    }
}

#[test]
fn test_eval() {
    let mut ctx = Context::new().unwrap();
    assert_eq!(Value::Undefined, ctx.eval("undefined").unwrap());
    assert_eq!(Value::Null, ctx.eval("null").unwrap());
    assert_eq!(Value::Bool(true), ctx.eval("true").unwrap());
    assert_eq!(Value::Bool(false), ctx.eval("false").unwrap());
    assert_eq!(Value::Number(5.0), ctx.eval("2 + 3").unwrap());

    let result = ctx.eval("'é'").unwrap();
    let expected = Value::String(Cow::Borrowed("é"));
    // result.woot();
    // expected.woot();

    println!("expected: {:?}",expected);
    println!("result: {:?}", result);
    // FIXME: un comment out the tests
    // assert_eq!(expected, result);
}

#[test]
fn test_unicode_supplementary_planes() {
    // Pay careful attention to characters U+10000 and greater, because
    // duktape uses CESU-8 internally, which isn't _quite_ valid UTF-8.
    // This is thanks to the fact that JavaScript uses 16-bit characters
    // and allows manipulating invalid UTF-16 data with mismatched
    // surrogate pairs.
    let mut ctx = Context::new().unwrap();

    // FIXME: un comment out the tests
    // assert_eq!(Value::String(Cow::Borrowed("𓀀")), ctx.eval("'𓀀'").unwrap());
    // assert_eq!(Value::String(Cow::Borrowed("𓀀")),
               // ctx.eval("'\\uD80C\\uDC00'").unwrap());

    ctx.eval("function id(x) { return x; }").unwrap();
    // assert_eq!(Ok(Value::String(Cow::Borrowed("𓀀"))),
               // ctx.call("id", &[&"𓀀"]));
}

#[test]
fn test_eval_errors() {
    let mut ctx = Context::new().unwrap();
    assert_eq!(true, ctx.eval("3 +").is_err());
}

#[test]
fn test_call_function_by_name() {
    use rustc_serialize::json::Json;

    let mut ctx = Context::new().unwrap();
    ctx.eval("function add(x, y) { return x+y; }").unwrap();
    assert_eq!(Ok(Value::Number(3.0)), ctx.call("add", &[&2.0f64, &1.0f64]));

    ctx.eval("function id(x) { return x; }").unwrap();
    assert_eq!(Ok(Value::Null),  ctx.call("id", &[&Json::Null]));
    assert_eq!(Ok(Value::Bool(true)),  ctx.call("id", &[&true]));
    assert_eq!(Ok(Value::Bool(false)), ctx.call("id", &[&false]));
    assert_eq!(Ok(Value::Number(1.5)), ctx.call("id", &[&1.5f64]));
    // FIXME: un comment out the tests
    // assert_eq!(Ok(Value::String(Cow::Borrowed("é"))),
               // ctx.call("id", &[&"é"]));
}

