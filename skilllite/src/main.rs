fn main() {
    if let Err(e) = skilllite::run_cli() {
        eprintln!("{e:?}");
        std::process::exit(1);
    }
}
