#[macro_use]
use duktape_sys::*;

/// To avoid massive debugging frustration, wrap stack manipulation code in
/// this macro.
macro_rules! assert_stack_height_unchanged {
    ($ctx:ident, $body:block) => {
        {
            let initial_stack_height = duk_get_top($ctx.ptr);
            let result = $body;
            assert_eq!(initial_stack_height, duk_get_top($ctx.ptr));
            result
        }
    }
}

macro_rules! assert_encode {
    ($val:expr) => {
        {
            let v = $val;
            let expected = ::rustc_serialize::json::encode(&v).unwrap();
            assert_json(&mut ctx, expected.as_slice(), &v);
        }
    }
}

macro_rules! assert_decode {
    ($val:expr) => { assert_decode(&mut ctx, &$val) }
}


macro_rules! read_and_convert {
    ($name:ident -> $ty:ident, $reader:ident -> $in_ty:ident) => {
        fn $name(&mut self) -> DuktapeResult<$ty> {
            self.$reader().map(|(_, v)| v as $ty)
        }
    }
}

macro_rules! read_with {
    ($name:ident -> $ty:ident, $tester:ident,
     |$slf:ident, $idx:ident| $reader:block) => {
        fn $name(&mut $slf) -> DuktapeResult<$ty> {
            unsafe {
                let $idx = -1;
                if $tester($slf.ctx.as_mut_ptr(), $idx) != 0 {
                    let result = $reader;
                    duk_pop($slf.ctx.as_mut_ptr());
                    result
                } else {
                    duk_pop($slf.ctx.as_mut_ptr());
                    Err(DuktapeError::from_str("Expected number"))
                }
            }
        }
    }
}
