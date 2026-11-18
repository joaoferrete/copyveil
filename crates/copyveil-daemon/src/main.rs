//! `copyveil-daemon`: see the library docs.

use std::io::IsTerminal;
use std::process::ExitCode;

#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .without_time()
        .with_target(false)
        .with_ansi(std::io::stderr().is_terminal())
        .with_writer(std::io::stderr)
        .init();
    if let Err(err) = copyveil_daemon::harden() {
        tracing::error!(%err, "could not make the process non-dumpable; refusing to start");
        return ExitCode::FAILURE;
    }
    match copyveil_daemon::run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            tracing::error!(%err, "copyveil-daemon failed");
            ExitCode::FAILURE
        }
    }
}
