#![forbid(unsafe_code)]

use quay_app::{paths, session};

use std::process::ExitCode;

fn main() -> ExitCode {
    let started = std::time::Instant::now();
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

    if arguments.is_empty() || arguments.first().map(String::as_str) == Some("ui") {
        return match quay_app::ui::run(started) {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("la fenêtre n'a pas démarré : {error}");
                ExitCode::FAILURE
            }
        };
    }

    let outcome = match arguments.first().map(String::as_str) {
        Some("login") => runtime.block_on(session::login()),
        Some("logout") => session::logout(),
        Some("sync") => runtime.block_on(session::sync()),
        Some("watch") => {
            let cycles = arguments.get(1).and_then(|value| value.parse().ok());
            runtime.block_on(session::watch(cycles))
        }
        Some("inbox") => session::inbox(),
        Some("views") => session::views(),
        Some("view") => match arguments.get(1) {
            Some(name) => session::view(name),
            None => return usage(),
        },
        Some("query") => {
            if arguments.len() < 2 {
                return usage();
            }
            session::query(&arguments[1..].join(" "))
        }
        Some("complete") => session::complete(&arguments[1..].join(" ")),
        Some("keys") => runtime.block_on(session::keys(arguments.get(1).map(String::as_str))),
        Some("show") => match arguments.get(1) {
            Some(locator) => session::show(locator),
            None => return usage(),
        },
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
    eprintln!("  ui       ouvrir la fenêtre (défaut sans argument)");
    eprintln!("  login    valider un jeton et le déposer dans le trousseau");
    eprintln!("  logout   retirer le jeton du trousseau et faire taire ses demandes");
    eprintln!("  sync     rafraîchir depuis la forge et écrire dans SQLite");
    eprintln!("  watch    [n] boucler la synchronisation au rythme annoncé par la forge");
    eprintln!("  inbox    afficher la file de revue depuis SQLite");
    eprintln!("  views    lister les vues sauvegardées et leurs raccourcis");
    eprintln!("  view     <nom> exécuter une vue sauvegardée");
    eprintln!("  query    <dsl> exécuter une requête ponctuelle");
    eprintln!("  complete <début> proposer la suite d'une requête");
    eprintln!("  keys     [portée] carte des touches valides ici");
    eprintln!("  show     <proprietaire/depot#numero> afficher une PR depuis SQLite");
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
