extern crate gcc;

use std::path::Path;
use std::env::{var, set_var};

fn main() {
    // Make sure we get a thread-safe build.  Without this, duktape refuses
    // to set DUK_USE_VARIADIC_MACROS and falls back to global variables.
    let mut cflags = var("CFLAGS").unwrap_or("".to_string());
    cflags.push_str(" -std=c99");
    set_var("CFLAGS", cflags);

    &gcc::Config::new()
        .file(Path::new("duktape/src/duktape.c"))
        .file(Path::new("src/glue.c"))
        .include("duktape/src")
        .compile("libduktape.a");
}
