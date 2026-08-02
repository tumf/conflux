use chrono::Utc;

fn main() {
    let build_time = Utc::now();
    let build_number = build_time.format("%Y%m%d%H%M%S").to_string();

    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);

    println!("cargo:rerun-if-changed=build.rs");
}
