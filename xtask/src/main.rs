fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    dictation_protocol::export_typescript(&root.join("bindings/node/generated"))?;
    cbindgen::Builder::new()
        .with_crate(root.join("bindings/c"))
        .with_language(cbindgen::Language::C)
        .with_include_guard("DICTATION_API_H")
        .generate()?
        .write_to_file(root.join("bindings/c/dictation_api.h"));
    Ok(())
}
