fn main() {
    if let Err(err) = slotforge::run() {
        eprintln!("SlotForge exited with error: {err}");
        std::process::exit(1);
    }
}
