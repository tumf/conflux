use chrono::Utc;
use std::path::Path;
use std::process::Command;

fn main() {
    // Generate UTC build number in YYYYMMDDHHmmss format
    let build_time = Utc::now();
    let build_number = build_time.format("%Y%m%d%H%M%S").to_string();

    println!("cargo:rustc-env=BUILD_NUMBER={}", build_number);

    // Build dashboard frontend if dashboard directory exists
    let dashboard_dir = Path::new("dashboard");
    if dashboard_dir.exists() {
        let build_script = if cfg!(target_os = "windows") {
            dashboard_dir.join("build.sh")
        } else {
            dashboard_dir.join("build.sh")
        };

        if build_script.exists() {
            println!("Building dashboard...");
            let output = if cfg!(target_os = "windows") {
                Command::new("bash")
                    .arg(build_script.to_str().unwrap())
                    .current_dir(&dashboard_dir)
                    .output()
            } else {
                Command::new("bash")
                    .arg(build_script.to_str().unwrap())
                    .current_dir(&dashboard_dir)
                    .output()
            };

            match output {
                Ok(output) => {
                    if !output.status.success() {
                        eprintln!(
                            "Dashboard build failed: {}",
                            String::from_utf8_lossy(&output.stderr)
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to run dashboard build: {}", e);
                }
            }
        }

        // Rerun if dashboard source changes
        println!("cargo:rerun-if-changed=dashboard/src");
        println!("cargo:rerun-if-changed=dashboard/package.json");
    }

    // Re-run if build script changes
    println!("cargo:rerun-if-changed=build.rs");
}
