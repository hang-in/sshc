use std::process::ExitCode;

use sshc::cli;
use sshc::tui::install_panic_hook;

fn main() -> ExitCode {
    env_logger::init();
    install_panic_hook();
    cli::run(std::env::args().collect())
}
