use chrono::Utc;

fn main() {
    // Generate UTC build number in YYYYMMDDHHmmss format
    let build_time = Utc::now();
    let build_number = build_time.format("%Y%m%d%H%M%S").to_string();

    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);

    // Re-run if build script changes
    println!("cargo:rerun-if-changed=build.rs");
}
