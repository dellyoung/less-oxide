fn main() {
    // 仅在启用 node 特性时配置 napi 生成。
    if std::env::var_os("CARGO_FEATURE_NODE").is_some() {
        napi_build::setup();
    }
    println!("cargo:rerun-if-changed=src/lib.rs");
    println!("cargo:rerun-if-changed=src/parser.rs");
    println!("cargo:rerun-if-changed=src/evaluator.rs");
    println!("cargo:rerun-if-changed=src/serializer.rs");
}
