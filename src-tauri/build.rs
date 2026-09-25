fn main() {
    tauri_build::build();
    // Note: To bundle skilllite binary, run before tauri build:
    //   cargo build -p skilllite && cp ../../target/release/skilllite src-tauri/resources/
    // Then add "resources": ["resources/**/*"] to bundle in tauri.conf.json
}
