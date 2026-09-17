use chrono::Utc;

fn main() {
    let build_time = Utc::now();
    let build_number = build_time.format("%Y%m%d%H%M%S").to_string();

    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);

    reserve_main_thread_stack();

    println!("cargo:rerun-if-changed=build.rs");
}

/// Reserve a usable main-thread stack for the Windows binary.
///
/// Windows reserves 1 MiB for a program's main thread, where POSIX targets give
/// 8 MiB, and `#[tokio::main]` drives the *whole* command future on that thread.
/// The generated state machine for a command like `cflx client status` — clap's
/// parse, the route resolution, and the observation loop, all inlined into one
/// future — does not fit in 1 MiB in a debug build, and a program that overflows
/// its stack exits with `STATUS_STACK_OVERFLOW` and no diagnostic at all.
///
/// Reserving is not committing: this asks the loader for address space, not for
/// memory, so the cost is a larger reservation in the binary header and nothing
/// at runtime. Applied to bin targets only — a library has no main thread to
/// size — and only on Windows, where the default is the one that is too small.
fn reserve_main_thread_stack() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    const STACK_BYTES: usize = 8 * 1024 * 1024;
    match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("gnu") => println!("cargo:rustc-link-arg-bins=-Wl,--stack,{STACK_BYTES}"),
        // MSVC, and anything else that speaks `link.exe` options.
        _ => println!("cargo:rustc-link-arg-bins=/STACK:{STACK_BYTES}"),
    }
}
