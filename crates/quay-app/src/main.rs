#![forbid(unsafe_code)]

mod paths;
mod render;
mod session;

use std::process::ExitCode;

fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .init();

    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let runtime = match tokio::runtime::Runtime::new() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("le runtime asynchrone n'a pas démarré : {error}");
            return ExitCode::FAILURE;
        }
    };

    let outcome = match arguments.first().map(String::as_str) {
        Some("login") => runtime.block_on(session::login()),
        Some("sync") => runtime.block_on(session::sync()),
        Some("inbox") => session::inbox(),
        Some("approve") => match arguments.get(1) {
            Some(locator) => session::approve(locator),
            None => return usage(),
        },
        Some("push") => runtime.block_on(session::push()),
        Some("cancel") => match arguments.get(1) {
            Some(identifier) => session::cancel(identifier),
            None => return usage(),
        },
        Some("status") => runtime.block_on(session::status()),
        _ => return usage(),
    };

    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("usage : quay <commande>");
    eprintln!("  login    valider un jeton et le déposer dans le trousseau");
    eprintln!("  sync     rafraîchir depuis la forge et écrire dans SQLite");
    eprintln!("  inbox    afficher la file de revue depuis SQLite");
    eprintln!("  approve  <proprietaire/depot#numero> approuver, en optimiste");
    eprintln!("  push     vider la file de mutations vers la forge");
    eprintln!("  cancel   <id> renoncer à une mutation et défaire son état optimiste");
    eprintln!("  status   quota, capacités, file de mutations et durabilité");
    eprintln!();
    eprintln!(
        "base de données : ${} ou l'emplacement de données du système",
        paths::DATABASE_VARIABLE
    );
    ExitCode::from(2)
}
