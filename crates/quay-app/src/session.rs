use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use quay_core::MutationState;
use quay_core::Priority;
use quay_forge::{
    Capabilities, Capability, DevLocks, GovernorConfig, Identity, OutboundRequest,
    PollingCheckpoint, PollingSource, RateGovernor, SearchSource, SingleSignOn, Support, Token,
    read_identity,
};
use quay_store::Store;
use quay_sync::SyncEngine;
use time::OffsetDateTime;

use crate::paths;
use crate::render;

const API: &str = "https://api.github.com";
const KEYCHAIN_SERVICE: &str = "quay";
const TOKEN_VARIABLE: &str = "QUAY_TEST_TOKEN";

type Outcome = Result<(), Box<dyn Error>>;

fn open_store() -> Result<Store, Box<dyn Error>> {
    let path = paths::database();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    Ok(Store::open(&path)?)
}

fn keychain() -> quay_forge::Keychain {
    quay_forge::Keychain::for_service(KEYCHAIN_SERVICE)
}

pub fn stored_login(store: &Store) -> Result<Option<String>, Box<dyn Error>> {
    let mut statement = store
        .connection()
        .prepare("SELECT login FROM account ORDER BY id LIMIT 1")?;
    let mut rows = statement.query_map([], |row| row.get::<_, String>(0))?;
    match rows.next() {
        None => Ok(None),
        Some(login) => Ok(Some(login?)),
    }
}

fn token_for(login: Option<&str>) -> Result<Token, Box<dyn Error>> {
    if let Some(token) = Token::from_env(TOKEN_VARIABLE) {
        return Ok(token);
    }
    match login {
        Some(login) => keychain()
            .load(login)?
            .ok_or_else(|| format!("aucun jeton pour {login} : lancer `quay login`").into()),
        None => Err(format!(
            "aucun compte connu : lancer `quay login`, ou définir ${TOKEN_VARIABLE}"
        )
        .into()),
    }
}

fn governor(token: Token) -> Result<Arc<RateGovernor>, Box<dyn Error>> {
    Ok(Arc::new(RateGovernor::new(
        GovernorConfig::default(),
        Some(token),
        DevLocks::from_env(),
    )?))
}

async fn identify(
    governor: &RateGovernor,
    kind: quay_forge::TokenKind,
) -> Result<Identity, Box<dyn Error>> {
    let user = governor
        .send(OutboundRequest::rest_read(
            format!("{API}/user"),
            Priority::User,
        ))
        .await?;
    let organizations = governor
        .send(OutboundRequest::rest_read(
            format!("{API}/user/orgs"),
            Priority::User,
        ))
        .await?;
    Ok(read_identity(kind, &user, &organizations)?)
}

pub async fn login() -> Outcome {
    let token = Token::from_env(TOKEN_VARIABLE).ok_or_else(|| {
        format!("définir ${TOKEN_VARIABLE} avant `quay login` ; le jeton n'est jamais lu depuis un fichier")
    })?;
    let kind = token.kind();
    let governor = governor(token.clone())?;
    let identity = identify(&governor, kind).await?;

    keychain().store(&identity.login, &token)?;
    let store = open_store()?;
    store.remember_account(
        "api.github.com",
        &identity.login,
        "pat_classic",
        &format!("{KEYCHAIN_SERVICE}/{}", identity.login),
    )?;

    println!("connecté en tant que {} ({})", identity.login, kind.label());
    println!("jeton déposé dans le trousseau, jamais écrit sur disque");
    println!(
        "{} organisation(s) visible(s)",
        identity.organizations.len()
    );
    report_single_sign_on(&identity);
    report_capabilities(&Capabilities::from_identity(&identity));
    Ok(())
}

fn report_single_sign_on(identity: &Identity) {
    match &identity.single_sign_on {
        SingleSignOn::NoRestriction => {}
        SingleSignOn::PartialResults { organization_ids } => println!(
            "attention : {} organisation(s) masquée(s) par SAML SSO, autoriser le jeton pour les voir",
            organization_ids.len()
        ),
        SingleSignOn::AuthorizationRequired { url } => {
            println!("attention : le jeton doit être autorisé pour cette organisation : {url}");
        }
    }
}

fn report_capabilities(capabilities: &Capabilities) {
    for capability in Capability::ALL {
        let support = capabilities.get(capability);
        let verdict = match support {
            Support::Available => "disponible".to_owned(),
            Support::Unknown => "inconnue, traitée comme disponible".to_owned(),
            Support::Unavailable { reason } => format!("indisponible : {reason:?}"),
        };
        println!("  {:<16} {verdict}", capability.id());
    }
}

struct Session {
    engine: SyncEngine,
    checkpoint: PollingCheckpoint,
    governor: Arc<RateGovernor>,
    login: String,
}

async fn open_session() -> Result<Session, Box<dyn Error>> {
    let store = open_store()?;
    let login = stored_login(&store)?;
    let token = token_for(login.as_deref())?;
    let kind = token.kind();
    let governor = governor(token)?;
    let identity = identify(&governor, kind).await?;
    let account_id = store.remember_account(
        "api.github.com",
        &identity.login,
        "pat_classic",
        &format!("{KEYCHAIN_SERVICE}/{}", identity.login),
    )?;

    let checkpoint = PollingCheckpoint::holding(quay_sync::notification_validators(&store)?);
    let engine = SyncEngine::new(store, governor.clone(), API, account_id)
        .listening_to(Box::new(PollingSource::sharing(
            governor.clone(),
            API,
            checkpoint.clone(),
        )))
        .listening_to(Box::new(SearchSource::review_queue(governor.clone(), API)));

    Ok(Session {
        engine,
        checkpoint,
        governor,
        login: identity.login,
    })
}

async fn cycle(session: &mut Session) -> Outcome {
    let started = Instant::now();
    let report = session.engine.tick().await?;
    quay_sync::remember_notification_validators(
        session.engine.store(),
        session.checkpoint.validators().as_ref(),
    )?;
    println!(
        "{} signal(s), {} déjà à jour, {} récupérée(s), {} écrite(s), {} échec(s), en {:.0} ms",
        report.signals,
        report.already_current,
        report.fetched,
        report.stored,
        report.failures.len(),
        started.elapsed().as_secs_f64() * 1_000.0
    );
    for failure in &report.failures {
        println!("  échec : {failure}");
    }

    let preload = session.engine.preload(&session.login).await?;
    if preload.speculation_disabled {
        println!("préchargement désactivé : son taux d'utilisation est passé sous le plancher");
    } else {
        println!(
            "préchargement : {} planifiée(s), {} déjà fraîche(s), {} récupérée(s), {} refusée(s) par le plafond",
            preload.planned, preload.already_fresh, preload.fetched, preload.refused_by_budget
        );
    }

    let mutations = session.engine.drain_mutations(32).await?;
    if mutations.sent + mutations.rolled_back + mutations.deferred > 0 {
        println!(
            "mutations : {} envoyée(s), {} annulée(s), {} différée(s)",
            mutations.sent, mutations.rolled_back, mutations.deferred
        );
    }

    println!(
        "{} requête(s), {} point(s), {} révalidation(s) gratuite(s), état {:?}",
        session.governor.issued_requests(),
        session.governor.spent_points(),
        session.governor.free_revalidations(),
        session.governor.health()
    );
    Ok(())
}

pub async fn sync() -> Outcome {
    let mut session = open_session().await?;
    cycle(&mut session).await
}

pub async fn watch(cycles: Option<usize>) -> Outcome {
    let mut session = open_session().await?;
    let mut completed = 0usize;
    loop {
        cycle(&mut session).await?;
        completed += 1;
        if cycles.is_some_and(|wanted| completed >= wanted) {
            return Ok(());
        }
        let interval = session.checkpoint.interval();
        println!("prochain passage dans {} s", interval.as_secs());
        tokio::time::sleep(interval).await;
    }
}

const DEFAULT_VIEW: &str = "À relire";
const RESULT_LIMIT: i64 = 200;

fn render_rows(rows: &[quay_store::inbox::InboxRow], elapsed: std::time::Duration, source: &str) {
    if rows.is_empty() {
        println!("{}", render::empty_inbox_invitation());
    } else {
        println!("{}", render::header());
        let palette = render::Palette::from_env();
        let now = OffsetDateTime::now_utc();
        for row in rows {
            println!("{}", render::row(row, &palette, now));
        }
    }
    println!();
    println!(
        "{} ligne(s) depuis SQLite en {:.3} ms — {source}",
        rows.len(),
        elapsed.as_secs_f64() * 1_000.0
    );
}

fn run_query(store: &Store, dsl: &str, viewer: &str, source: &str) -> Outcome {
    let parsed = quay_core::parse_query(dsl).map_err(|error| render::query_error(&error))?;
    let started = Instant::now();
    let rows = store
        .search(&parsed, viewer, RESULT_LIMIT)
        .map_err(|error| render::store_error(&error).unwrap_or_else(|| error.to_string()))?;
    let elapsed = started.elapsed();
    render_rows(&rows, elapsed, source);
    store.note_navigation_event(
        "shell",
        "inbox:list",
        "cmd:view.open",
        None,
        None,
        now_seconds(),
    )?;
    Ok(())
}

pub fn inbox() -> Outcome {
    let store = open_store()?;
    store.install_shipped_views()?;
    let viewer = stored_login(&store)?.unwrap_or_default();
    match store.view(DEFAULT_VIEW)? {
        Some(view) => run_query(
            &store,
            &view.query,
            &viewer,
            &format!("vue « {} »", view.name),
        ),
        None => {
            Err(format!("la vue « {DEFAULT_VIEW} » a disparu, `quay views` la réinstalle").into())
        }
    }
}

pub fn views() -> Outcome {
    let store = open_store()?;
    let installed = store.install_shipped_views()?;
    if installed > 0 {
        println!("{installed} vue(s) par défaut installée(s)");
    }
    let views = store.views()?;
    if views.is_empty() {
        println!("aucune vue sauvegardée");
        return Ok(());
    }
    for view in &views {
        println!(
            "{:<24}  {:<5}  {}",
            view.name,
            view.shortcut.as_deref().unwrap_or("—"),
            view.query
        );
    }
    Ok(())
}

pub fn view(name: &str) -> Outcome {
    let store = open_store()?;
    store.install_shipped_views()?;
    let viewer = stored_login(&store)?.unwrap_or_default();
    let view = store
        .view(name)?
        .ok_or_else(|| format!("aucune vue nommée « {name} », `quay views` les liste"))?;
    run_query(
        &store,
        &view.query,
        &viewer,
        &format!("vue « {} »", view.name),
    )
}

pub fn query(dsl: &str) -> Outcome {
    let store = open_store()?;
    let viewer = stored_login(&store)?.unwrap_or_default();
    run_query(&store, dsl, &viewer, "requête ponctuelle")
}

pub async fn keys(scope: Option<&str>) -> Outcome {
    let store = open_store()?;
    let login = stored_login(&store)?;
    let capabilities = match token_for(login.as_deref()) {
        Ok(token) => {
            let kind = token.kind();
            let governor = governor(token)?;
            match identify(&governor, kind).await {
                Ok(identity) => Capabilities::from_identity(&identity),
                Err(_) => Capabilities::from_scopes(kind, &Default::default()),
            }
        }
        Err(_) => {
            Capabilities::from_scopes(quay_forge::TokenKind::Unrecognised, &Default::default())
        }
    };

    let wanted = scope.and_then(|name| {
        crate::commands::Scope::ALL
            .into_iter()
            .find(|candidate| candidate.id() == name)
    });
    let scopes: Vec<crate::commands::Scope> = match wanted {
        Some(scope) => vec![scope],
        None => crate::commands::Scope::ALL.to_vec(),
    };

    for scope in scopes {
        println!("— {} —", scope.id());
        for command in crate::commands::available_in(scope, &capabilities) {
            let bindings = command
                .chords()
                .iter()
                .map(crate::keys::KeyChord::render)
                .collect::<Vec<String>>()
                .join(" / ");
            let state = if command.is_enabled(&capabilities) {
                String::new()
            } else {
                match command.requires {
                    Some(capability) => format!("  (indisponible : {})", capability.id()),
                    None => String::new(),
                }
            };
            println!("  {bindings:<12} {}{state}", command.title);
        }
        println!();
    }
    Ok(())
}

pub fn complete(partial: &str) -> Outcome {
    for completion in quay_core::completions(partial) {
        println!("{}", completion.insertion);
    }
    Ok(())
}

fn parse_locator(locator: &str) -> Result<(String, String, i64), Box<dyn Error>> {
    let (repository, number) = locator
        .rsplit_once('#')
        .ok_or("attendu : proprietaire/depot#numero")?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or("attendu : proprietaire/depot#numero")?;
    let number: i64 = number
        .parse()
        .map_err(|_| "le numéro doit être un entier")?;
    Ok((owner.to_owned(), name.to_owned(), number))
}

pub fn show(locator: &str) -> Outcome {
    let (owner, name, number) = parse_locator(locator)?;
    let store = open_store()?;

    let started = Instant::now();
    let view = store.pull_request_view(&owner, &name, number)?;
    let elapsed = started.elapsed();

    let key = format!("{owner}/{name}#{number}");
    let now = now_seconds();
    store.note_navigation(&key, view.is_some(), now)?;
    let was_preloaded = store.note_speculation_used(&key)?;
    store.note_navigation_event(
        "inbox:list",
        "pr:detail",
        "cmd:pr.open",
        Some("pr"),
        None,
        now,
    )?;

    let Some(view) = view else {
        println!("{locator} n'est pas dans la base locale, lancer `quay sync`");
        return Ok(());
    };

    println!(
        "{}/{}#{} — {}",
        view.owner, view.name, view.number, view.title
    );
    println!(
        "  {} par {}, review {}, checks {}, {} thread(s) non résolu(s)",
        view.state,
        view.author,
        view.review_state.as_deref().unwrap_or("aucune"),
        view.checks_state.as_deref().unwrap_or("aucun"),
        view.unresolved_threads()
    );
    if view.threads.is_empty() {
        println!("  aucun thread de review");
    }
    for thread in &view.threads {
        let mark = if thread.is_resolved {
            "résolu "
        } else {
            "ouvert "
        };
        let outdated = if thread.is_outdated {
            " (obsolète)"
        } else {
            ""
        };
        println!(
            "  [{mark}] {}:{}{outdated}",
            thread.path,
            thread
                .line
                .map(|line| line.to_string())
                .unwrap_or_else(|| "?".to_owned())
        );
        for comment in &thread.comments {
            println!("      {} — {}", comment.author, first_line(&comment.body));
        }
    }

    println!();
    println!(
        "servi depuis SQLite en {:.3} ms{}",
        elapsed.as_secs_f64() * 1_000.0,
        if was_preloaded { ", préchargé" } else { "" }
    );
    Ok(())
}

fn first_line(body: &str) -> String {
    let line = body.lines().next().unwrap_or_default();
    if line.chars().count() > 72 {
        format!("{}…", line.chars().take(71).collect::<String>())
    } else {
        line.to_owned()
    }
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

pub fn approve(locator: &str) -> Outcome {
    let (owner, name, number) = parse_locator(locator)?;
    let mut store = open_store()?;
    let node_id = store
        .find_pull_request(&owner, &name, number)?
        .ok_or_else(|| format!("{locator} n'est pas dans la base locale, lancer `quay sync`"))?;

    let now = now_seconds();
    let idempotency = format!("approve_pr:{node_id}:{now}");
    let identifier = store.approve_pull_request(&node_id, &idempotency, now)?;
    println!("approbation {identifier} en file, état local déjà à jour");
    println!("`quay push` pour l'envoyer ; le mode lecture seule la garde en attente");
    Ok(())
}

pub fn cancel(identifier: &str) -> Outcome {
    let identifier: i64 = identifier
        .parse()
        .map_err(|_| "attendu : l'identifiant numérique de la mutation")?;
    let mut store = open_store()?;
    let mutation = store
        .mutation(identifier)?
        .ok_or_else(|| format!("aucune mutation {identifier}"))?;
    store.roll_back_mutation(identifier, "annulée par l'utilisateur")?;
    println!(
        "{} sur {} annulée, état local revenu à ce qu'il était",
        mutation.kind.id(),
        mutation.target
    );
    Ok(())
}

pub async fn push() -> Outcome {
    let mut store = open_store()?;
    let replay = store.replay_interrupted_mutations()?;
    if replay.requeued > 0 || replay.held_back > 0 {
        println!(
            "reprise : {} remise(s) en file, {} retenue(s) car un renvoi créerait un doublon",
            replay.requeued, replay.held_back
        );
    }

    let login = stored_login(&store)?;
    let token = token_for(login.as_deref())?;
    let governor = governor(token)?;
    let report = quay_sync::drain_mutations(&mut store, &governor, API, 32).await?;
    println!(
        "{} envoyée(s), {} annulée(s), {} différée(s)",
        report.sent, report.rolled_back, report.deferred
    );
    for failure in &report.failures {
        println!("  annulée : {failure}");
    }
    print_queue(&store)?;
    Ok(())
}

fn print_queue(store: &Store) -> Outcome {
    for state in [MutationState::Pending, MutationState::Failed] {
        let rows = store.mutations_in_state(state)?;
        if rows.is_empty() {
            continue;
        }
        println!("file {} : {} entrée(s)", state.id(), rows.len());
        for mutation in rows.iter().take(5) {
            println!(
                "  {} sur {}, {} tentative(s){}",
                mutation.kind.id(),
                mutation.target,
                mutation.attempts,
                mutation
                    .last_error
                    .as_deref()
                    .map(|reason| format!(" — {reason}"))
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}

pub async fn status() -> Outcome {
    let store = open_store()?;
    let login = stored_login(&store)?;
    let token = token_for(login.as_deref())?;
    let kind = token.kind();
    let governor = governor(token)?;
    let identity = identify(&governor, kind).await?;

    println!("compte    {} ({})", identity.login, kind.label());
    println!("scopes    {}", identity.scopes.listed().join(", "));
    report_single_sign_on(&identity);
    match governor.rate_limit() {
        Some(snapshot) => println!(
            "quota     {} restant sur {}, état {:?}",
            snapshot.remaining,
            snapshot.limit,
            governor.health()
        ),
        None => println!("quota     non rapporté par la forge"),
    }

    let stored: i64 = store.connection().query_row(
        "SELECT COUNT(*) FROM pull_request WHERE state = 'open'",
        [],
        |row| row.get(0),
    )?;
    let repositories: i64 =
        store
            .connection()
            .query_row("SELECT COUNT(*) FROM repo", [], |row| row.get(0))?;
    println!("local     {stored} pull request(s) ouverte(s) sur {repositories} dépôt(s)");
    println!("base      {}", store.path().display());
    println!("durabilité {:?}", store.durability());
    let speculation = store.speculation_utilisation(500)?;
    let navigations = store.navigations_served_locally(500)?;
    println!(
        "préchargement : {}/{} utilisé(s){}",
        speculation.used,
        speculation.observed,
        speculation
            .percent()
            .map(|percent| format!(", soit {percent:.0} %"))
            .unwrap_or_default()
    );
    println!(
        "navigations servies localement : {}/{}{}",
        navigations.used,
        navigations.observed,
        navigations
            .percent()
            .map(|percent| format!(", soit {percent:.0} %"))
            .unwrap_or_default()
    );
    print_queue(&store)?;
    report_capabilities(&Capabilities::from_identity(&identity));
    Ok(())
}
