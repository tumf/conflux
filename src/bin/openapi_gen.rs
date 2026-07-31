//! OpenAPI specification generator for the Conflux remote-control API.

fn main() {
    let yaml = serde_yaml::to_string(&conflux::web::openapi::document())
        .expect("Failed to serialize OpenAPI spec to YAML");
    println!("{}", yaml.trim_end());
}
