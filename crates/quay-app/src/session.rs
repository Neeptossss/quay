use std::error::Error;
use std::sync::Arc;
use std::time::Instant;

use quay_core::MutationState;
use quay_core::Priority;
use quay_forge::{
    Capabilities, Capability, DevLocks, GovernorConfig, Identity, OutboundRequest, PollingSource,
    RateGovernor, SearchSource, SharedValidators, SingleSignOn, Support, Token, read_identity,
};
use quay_store::Store;
use quay_store::inbox::{InboxFilter, InboxQuery};
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

fn stored_login(store: &Store) -> Result<Option<String>, Box<dyn Error>> {
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

pub async fn sync() -> Outcome {
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

    let validators = SharedValidators::holding(quay_sync::notification_validators(&store)?);
    let polling = PollingSource::sharing(governor.clone(), API, validators.clone());
    let mut engine = SyncEngine::new(store, governor.clone(), API, account_id)
        .listening_to(Box::new(polling))
        .listening_to(Box::new(SearchSource::review_queue(governor.clone(), API)));

    let started = Instant::now();
    let report = engine.tick().await?;
    quay_sync::remember_notification_validators(engine.store(), validators.current().as_ref())?;
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
    println!(
        "{} requête(s), {} point(s), {} révalidation(s) gratuite(s), état {:?}",
        governor.issued_requests(),
        governor.spent_points(),
        governor.free_revalidations(),
        governor.health()
    );
    Ok(())
}

pub fn inbox() -> Outcome {
    let store = open_store()?;
    let login = stored_login(&store)?;
    let filter = InboxFilter {
        viewer: login.clone().unwrap_or_default(),
        ..InboxFilter::default()
    };

    let query = if login.is_some() {
        InboxQuery::ReviewRequestedExact
    } else {
        InboxQuery::OpenWithUnresolvedThreadCount
    };
    let started = Instant::now();
    let rows = store.inbox(query, &filter)?;
    let requested = started.elapsed();

    let started = Instant::now();
    let fallback = store.inbox(InboxQuery::OpenWithUnresolvedThreadCount, &filter)?;
    let everything = started.elapsed();

    let palette = render::Palette::from_env();
    let now = OffsetDateTime::now_utc();
    let shown = if rows.is_empty() { &fallback } else { &rows };

    if shown.is_empty() {
        println!("{}", render::empty_inbox_invitation());
    } else {
        println!("{}", render::header());
        for row in shown {
            println!("{}", render::row(row, &palette, now));
        }
    }

    println!();
    println!(
        "{} ligne(s) demandée(s) en {:.3} ms, {} ligne(s) ouverte(s) en {:.3} ms, depuis SQLite",
        rows.len(),
        requested.as_secs_f64() * 1_000.0,
        fallback.len(),
        everything.as_secs_f64() * 1_000.0
    );
    Ok(())
}

fn now_seconds() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

pub fn approve(locator: &str) -> Outcome {
    let (repository, number) = locator
        .rsplit_once('#')
        .ok_or("attendu : proprietaire/depot#numero")?;
    let (owner, name) = repository
        .split_once('/')
        .ok_or("attendu : proprietaire/depot#numero")?;
    let number: i64 = number
        .parse()
        .map_err(|_| "le numéro doit être un entier")?;

    let mut store = open_store()?;
    let node_id = store
        .find_pull_request(owner, name, number)?
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
    print_queue(&store)?;
    report_capabilities(&Capabilities::from_identity(&identity));
    Ok(())
}
