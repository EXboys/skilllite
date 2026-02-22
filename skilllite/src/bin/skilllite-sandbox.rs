//! skilllite-sandbox binary — sandbox + MCP only, no agent.
//! Built with: cargo build -p skilllite --bin skilllite-sandbox --no-default-features --features sandbox_binary

fn main() {
    if let Err(e) = skilllite::run_cli() {
        eprintln!("{e:?}");
        std::process::exit(1);
    }
}
