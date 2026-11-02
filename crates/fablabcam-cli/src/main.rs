fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match fablabcam_cli::run(&args) {
        Ok(out) => print!("{out}"),
        Err(e) => {
            eprintln!("erreur : {e}");
            std::process::exit(1);
        }
    }
}
