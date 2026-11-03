fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().is_some_and(|s| s == "machine") {
        match fablabcam_cli::machine::run(&args[1..]) {
            Ok(out) => { print!("{}", out.text); std::process::exit(out.exit_code); }
            Err(e) => { eprintln!("erreur : {e}"); std::process::exit(1); }
        }
    }
    match fablabcam_cli::run(&args) {
        Ok(out) => print!("{out}"),
        Err(e) => {
            eprintln!("erreur : {e}");
            std::process::exit(1);
        }
    }
}
