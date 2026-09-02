//! `mongol-norm` command-line entry point; the implementation is `mongol_norm::cli`.

fn main() {
    std::process::exit(mongol_norm::cli::main());
}
