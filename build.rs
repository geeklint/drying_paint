/* SPDX-License-Identifier: (Apache-2.0 OR MIT OR Zlib) */
/* Copyright © 2023 Violet Leonard */

use std::env::{self, VarError};

fn main() {
    let var_name = "DRYING_PAINT_WATCH_CYCLE_DEBUG";
    println!("cargo:rerun-if-env-changed={}", var_name);
    let do_cycle_debug = match env::var(var_name).as_deref() {
        Ok("always") => true,
        Ok("never") => false,
        Ok(value) => {
            panic!("unknown value for env '{}': '{}'", var_name, value)
        }
        Err(VarError::NotUnicode(value)) => panic!(
            "unknown value for env '{}': '{}'",
            var_name,
            value.to_string_lossy()
        ),
        Err(VarError::NotPresent) => cfg!(debug_assertions),
    };
    println!("cargo::rustc-check-cfg=cfg(do_cycle_debug)");
    if do_cycle_debug {
        println!("cargo:rustc-cfg=do_cycle_debug");
    }
}
