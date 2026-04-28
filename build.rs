use chrono::Utc;
use std::path::Path;

fn main() {
    let build_time = Utc::now();
    let build_number = build_time.format("%Y%m%d%H%M%S").to_string();

    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);

    let dashboard_dir = Path::new("dashboard");
    if dashboard_dir.exists() {
        let assets_dir = Path::new("dashboard/dist/assets");
        if let Ok(entries) = std::fs::read_dir(assets_dir) {
            for entry in entries.flatten() {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                if name.ends_with(".js") {
                    println!("cargo:rustc-env=DASHBOARD_JS={}", name);
                } else if name.ends_with(".css") {
                    println!("cargo:rustc-env=DASHBOARD_CSS={}", name);
                }
            }
        }

        println!("cargo:rerun-if-changed=dashboard/dist/assets");
        println!("cargo:rerun-if-changed=dashboard/dist/index.html");
        println!("cargo:rerun-if-changed=dashboard/dist/favicon.svg");
        println!("cargo:rerun-if-changed=dashboard/dist/icons.svg");
    }

    println!("cargo:rerun-if-changed=build.rs");
}
