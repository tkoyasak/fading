mod entry;

fn main() {
    let arg = std::env::args().nth(1);
    let _ = entry::generate_monthly_entry(arg).inspect_err(|e| {
        eprintln!("{e}");
    });
}
