fn main() {
    std::process::exit(smith_installer::cli::main(std::env::args_os().skip(1)));
}
