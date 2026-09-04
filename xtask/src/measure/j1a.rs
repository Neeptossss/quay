use std::error::Error;
use std::io::Write;
use std::time::Instant;

use quay_core::Priority;
use quay_forge::{
    DevLocks, ForgeResponse, GovernorConfig, OutboundRequest, RateGovernor, RequestKind, Token,
};
use serde_json::{Value, json};

use crate::paths;

const ENDPOINT: &str = "https://api.github.com/graphql";
const CANDIDATE_REPOSITORIES: [&str; 12] = [
    "rust-lang/rust",
    "kubernetes/kubernetes",
    "golang/go",
    "microsoft/vscode",
    "denoland/deno",
    "elastic/elasticsearch",
    "NixOS/nixpkgs",
    "home-assistant/core",
    "dotnet/runtime",
    "pytorch/pytorch",
    "apache/spark",
    "odoo/odoo",
];
const TARGET_FILE_COUNTS: [u64; 3] = [5, 50, 300];
const REPETITIONS: usize = 7;
const PAGE_CONFIGURATIONS: [(&str, u64, u64, u64); 3] = [
    ("small_pages", 20, 20, 10),
    ("medium_pages", 50, 50, 25),
    ("max_pages", 100, 100, 100),
];

const CANDIDATES_QUERY: &str = "\
query Candidates($search: String!, $count: Int!) {
  rateLimit { limit cost remaining nodeCount }
  search(query: $search, type: ISSUE, first: $count) {
    nodes {
      ... on PullRequest {
        number
        changedFiles
        repository { nameWithOwner }
        reviewThreads { totalCount }
      }
    }
  }
}";

const DETAIL_QUERY: &str = "\
query PrDetail($owner: String!, $name: String!, $number: Int!, $files: Int!, $threads: Int!, $comments: Int!) {
  rateLimit { limit cost remaining nodeCount }
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      number title state isDraft mergeable reviewDecision updatedAt baseRefName headRefOid
      author { login }
      files(first: $files) {
        totalCount
        pageInfo { hasNextPage endCursor }
        nodes { path additions deletions }
      }
      reviewThreads(first: $threads) {
        totalCount
        pageInfo { hasNextPage }
        nodes {
          id isResolved isOutdated path line originalLine diffSide
          comments(first: $comments) {
            totalCount
            nodes { id body createdAt author { login } }
          }
        }
      }
      commits(last: 1) { nodes { commit { oid statusCheckRollup { state } } } }
    }
  }
}";

const FILES_PAGE_QUERY: &str = "\
query PrFilesPage($owner: String!, $name: String!, $number: Int!, $files: Int!, $after: String) {
  rateLimit { limit cost remaining nodeCount }
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      files(first: $files, after: $after) {
        totalCount
        pageInfo { hasNextPage endCursor }
        nodes { path additions deletions }
      }
    }
  }
}";

const THREADS_PAGE_QUERY: &str = "\
query PrThreadsPage($owner: String!, $name: String!, $number: Int!, $threads: Int!, $comments: Int!, $after: String) {
  rateLimit { limit cost remaining nodeCount }
  repository(owner: $owner, name: $name) {
    pullRequest(number: $number) {
      reviewThreads(first: $threads, after: $after) {
        totalCount
        pageInfo { hasNextPage endCursor }
        nodes {
          id isResolved isOutdated path line originalLine diffSide
          comments(first: $comments) {
            totalCount
            nodes { id body createdAt author { login } }
          }
        }
      }
    }
  }
}";

#[derive(Debug, Clone)]
pub struct Target {
    pub owner: String,
    pub name: String,
    pub number: u64,
    pub changed_files: u64,
    pub review_threads: u64,
    pub selection: String,
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub target: Target,
    pub mode: String,
    pub configuration: String,
    pub repetition: usize,
    pub status: u16,
    pub elapsed_ms: f64,
    pub bytes: usize,
    pub requests: usize,
    pub cost: u64,
    pub node_count: u64,
    pub rate_limit_remaining: u64,
    pub files_returned: u64,
    pub files_total: u64,
    pub files_truncated: bool,
    pub threads_returned: u64,
    pub threads_total: u64,
    pub threads_truncated: bool,
    pub graphql_errors: Vec<String>,
}

pub async fn measure(started_at: &str) -> Result<Vec<Sample>, Box<dyn Error>> {
    let token = Token::from_env("QUAY_TEST_TOKEN")
        .ok_or("QUAY_TEST_TOKEN is not set; J1-a cannot be measured")?;
    let governor = RateGovernor::new(GovernorConfig::default(), Some(token), DevLocks::from_env())?;

    std::fs::create_dir_all(paths::raw())?;
    let targets = select_targets(&governor).await?;
    for target in &targets {
        println!(
            "j1a: target {} {}/{}#{} changed_files={} review_threads={}",
            target.selection,
            target.owner,
            target.name,
            target.number,
            target.changed_files,
            target.review_threads
        );
    }

    let file_name = format!("j1a-{}.jsonl", started_at.replace(':', ""));
    let mut log = std::fs::File::create(paths::raw().join(&file_name))?;
    let mut samples = Vec::new();

    for target in &targets {
        for (configuration, files, threads, comments) in PAGE_CONFIGURATIONS {
            for repetition in 0..REPETITIONS {
                let sample = single_shot(
                    &governor,
                    target,
                    configuration,
                    files,
                    threads,
                    comments,
                    repetition,
                )
                .await?;
                write_sample(&mut log, started_at, &sample)?;
                samples.push(sample);
            }
        }
        for repetition in 0..REPETITIONS {
            let sample = paginated(&governor, target, repetition).await?;
            write_sample(&mut log, started_at, &sample)?;
            samples.push(sample);
        }
    }

    println!(
        "j1a: {} requests issued, {} points spent, health {:?}",
        governor.issued_requests(),
        governor.spent_points(),
        governor.health()
    );
    Ok(samples)
}

async fn post(
    governor: &RateGovernor,
    body: Value,
) -> Result<(ForgeResponse, Value), Box<dyn Error>> {
    let request = OutboundRequest::graphql_query(ENDPOINT, body.to_string(), Priority::User);
    debug_assert_eq!(request.kind, RequestKind::GraphQlQuery);
    let response = governor.send(request).await?;
    let parsed: Value = serde_json::from_str(&response.body).unwrap_or_else(
        |error| json!({ "errors": [{ "message": format!("invalid JSON body: {error}") }] }),
    );
    Ok((response, parsed))
}

async fn select_targets(governor: &RateGovernor) -> Result<Vec<Target>, Box<dyn Error>> {
    let mut candidates: Vec<Target> = Vec::new();
    for repository in CANDIDATE_REPOSITORIES {
        let body = json!({
            "query": CANDIDATES_QUERY,
            "variables": {
                "search": format!("repo:{repository} is:pr is:merged sort:updated-desc"),
                "count": 100,
            }
        });
        let (response, parsed) = post(governor, body).await?;
        if response.status != 200 {
            println!(
                "j1a: candidate scan of {repository} returned {}",
                response.status
            );
            continue;
        }
        for error in graphql_errors(&parsed) {
            println!("j1a: candidate scan of {repository} reported: {error}");
        }
        let nodes = parsed
            .pointer("/data/search/nodes")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for node in nodes {
            let Some(number) = node.get("number").and_then(Value::as_u64) else {
                continue;
            };
            let Some(changed_files) = node.get("changedFiles").and_then(Value::as_u64) else {
                continue;
            };
            let name_with_owner = node
                .pointer("/repository/nameWithOwner")
                .and_then(Value::as_str)
                .unwrap_or(repository);
            let (owner, name) = name_with_owner.split_once('/').unwrap_or((repository, ""));
            candidates.push(Target {
                owner: owner.to_owned(),
                name: name.to_owned(),
                number,
                changed_files,
                review_threads: node
                    .pointer("/reviewThreads/totalCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0),
                selection: String::new(),
            });
        }
    }

    if candidates.is_empty() {
        return Err("no pull request candidate could be listed".into());
    }

    let mut targets: Vec<Target> = Vec::new();
    for wanted in TARGET_FILE_COUNTS {
        let chosen = candidates
            .iter()
            .filter(|candidate| !already_selected(&targets, candidate))
            .min_by_key(|candidate| candidate.changed_files.abs_diff(wanted));
        if let Some(chosen) = chosen {
            targets.push(Target {
                selection: format!("files~{wanted}"),
                ..chosen.clone()
            });
        }
    }

    let thread_heavy = candidates
        .iter()
        .filter(|candidate| !already_selected(&targets, candidate))
        .max_by_key(|candidate| candidate.review_threads);
    if let Some(thread_heavy) = thread_heavy {
        targets.push(Target {
            selection: "threads-max".to_owned(),
            ..thread_heavy.clone()
        });
    }
    Ok(targets)
}

fn already_selected(targets: &[Target], candidate: &Target) -> bool {
    targets.iter().any(|target| {
        target.number == candidate.number
            && target.name == candidate.name
            && target.owner == candidate.owner
    })
}

#[allow(clippy::too_many_arguments)]
async fn single_shot(
    governor: &RateGovernor,
    target: &Target,
    configuration: &str,
    files: u64,
    threads: u64,
    comments: u64,
    repetition: usize,
) -> Result<Sample, Box<dyn Error>> {
    let body = json!({
        "query": DETAIL_QUERY,
        "variables": {
            "owner": target.owner,
            "name": target.name,
            "number": target.number,
            "files": files,
            "threads": threads,
            "comments": comments,
        }
    });
    let started = Instant::now();
    let (response, parsed) = post(governor, body).await?;
    let elapsed_ms = started.elapsed().as_secs_f64() * 1_000.0;

    let files_node = parsed.pointer("/data/repository/pullRequest/files");
    let threads_node = parsed.pointer("/data/repository/pullRequest/reviewThreads");
    Ok(Sample {
        target: target.clone(),
        mode: "single_shot".to_owned(),
        configuration: configuration.to_owned(),
        repetition,
        status: response.status,
        elapsed_ms,
        bytes: response.bytes,
        requests: 1,
        cost: rate_limit_field(&parsed, "cost"),
        node_count: rate_limit_field(&parsed, "nodeCount"),
        rate_limit_remaining: rate_limit_field(&parsed, "remaining"),
        files_returned: connection_length(files_node),
        files_total: connection_total(files_node),
        files_truncated: connection_has_next_page(files_node),
        threads_returned: connection_length(threads_node),
        threads_total: connection_total(threads_node),
        threads_truncated: connection_has_next_page(threads_node),
        graphql_errors: graphql_errors(&parsed),
    })
}

struct PageWalk {
    requests: usize,
    bytes: usize,
    cost: u64,
    node_count: u64,
    remaining: u64,
    status: u16,
    returned: u64,
    total: u64,
    errors: Vec<String>,
}

async fn walk_pages<F>(
    governor: &RateGovernor,
    query: &str,
    pointer: &str,
    variables: F,
) -> Result<PageWalk, Box<dyn Error>>
where
    F: Fn(Option<String>) -> Value,
{
    let mut walk = PageWalk {
        requests: 0,
        bytes: 0,
        cost: 0,
        node_count: 0,
        remaining: 0,
        status: 0,
        returned: 0,
        total: 0,
        errors: Vec::new(),
    };
    let mut cursor: Option<String> = None;

    loop {
        let body = json!({ "query": query, "variables": variables(cursor.clone()) });
        let (response, parsed) = post(governor, body).await?;
        walk.requests += 1;
        walk.bytes += response.bytes;
        walk.status = response.status;
        walk.cost += rate_limit_field(&parsed, "cost");
        walk.node_count += rate_limit_field(&parsed, "nodeCount");
        walk.remaining = rate_limit_field(&parsed, "remaining");
        walk.errors.extend(graphql_errors(&parsed));

        let node = parsed.pointer(pointer);
        walk.returned += connection_length(node);
        walk.total = connection_total(node);
        if !connection_has_next_page(node) {
            return Ok(walk);
        }
        cursor = connection_end_cursor(node);
        if cursor.is_none() {
            return Ok(walk);
        }
    }
}

async fn paginated(
    governor: &RateGovernor,
    target: &Target,
    repetition: usize,
) -> Result<Sample, Box<dyn Error>> {
    let started = Instant::now();

    let files = walk_pages(
        governor,
        FILES_PAGE_QUERY,
        "/data/repository/pullRequest/files",
        |after| {
            json!({
                "owner": target.owner,
                "name": target.name,
                "number": target.number,
                "files": 100,
                "after": after,
            })
        },
    )
    .await?;

    let threads = walk_pages(
        governor,
        THREADS_PAGE_QUERY,
        "/data/repository/pullRequest/reviewThreads",
        |after| {
            json!({
                "owner": target.owner,
                "name": target.name,
                "number": target.number,
                "threads": 50,
                "comments": 25,
                "after": after,
            })
        },
    )
    .await?;

    let mut errors = files.errors;
    errors.extend(threads.errors);

    Ok(Sample {
        target: target.clone(),
        mode: "paginated".to_owned(),
        configuration: "files100_threads50_comments25".to_owned(),
        repetition,
        status: if files.status == 200 {
            threads.status
        } else {
            files.status
        },
        elapsed_ms: started.elapsed().as_secs_f64() * 1_000.0,
        bytes: files.bytes + threads.bytes,
        requests: files.requests + threads.requests,
        cost: files.cost + threads.cost,
        node_count: files.node_count + threads.node_count,
        rate_limit_remaining: threads.remaining,
        files_returned: files.returned,
        files_total: files.total,
        files_truncated: false,
        threads_returned: threads.returned,
        threads_total: threads.total,
        threads_truncated: false,
        graphql_errors: errors,
    })
}

fn rate_limit_field(parsed: &Value, field: &str) -> u64 {
    parsed
        .pointer(&format!("/data/rateLimit/{field}"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn connection_length(node: Option<&Value>) -> u64 {
    node.and_then(|node| node.get("nodes"))
        .and_then(Value::as_array)
        .map(|nodes| nodes.len() as u64)
        .unwrap_or(0)
}

fn connection_total(node: Option<&Value>) -> u64 {
    node.and_then(|node| node.get("totalCount"))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

fn connection_has_next_page(node: Option<&Value>) -> bool {
    node.and_then(|node| node.pointer("/pageInfo/hasNextPage"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn connection_end_cursor(node: Option<&Value>) -> Option<String> {
    node.and_then(|node| node.pointer("/pageInfo/endCursor"))
        .and_then(Value::as_str)
        .map(str::to_owned)
}

fn graphql_errors(parsed: &Value) -> Vec<String> {
    parsed
        .get("errors")
        .and_then(Value::as_array)
        .map(|errors| {
            errors
                .iter()
                .map(|error| {
                    error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("unnamed error")
                        .to_owned()
                })
                .collect()
        })
        .unwrap_or_default()
}

fn write_sample(
    log: &mut std::fs::File,
    started_at: &str,
    sample: &Sample,
) -> Result<(), Box<dyn Error>> {
    let line = json!({
        "scenario": "j1a",
        "run_started_at": started_at,
        "machine": crate::machine(),
        "repository": format!("{}/{}", sample.target.owner, sample.target.name),
        "pull_request": sample.target.number,
        "selection": sample.target.selection,
        "changed_files": sample.target.changed_files,
        "review_threads": sample.target.review_threads,
        "mode": sample.mode,
        "configuration": sample.configuration,
        "repetition": sample.repetition,
        "status": sample.status,
        "elapsed_ms": sample.elapsed_ms,
        "bytes": sample.bytes,
        "requests": sample.requests,
        "graphql_cost": sample.cost,
        "graphql_node_count": sample.node_count,
        "rate_limit_remaining": sample.rate_limit_remaining,
        "files_returned": sample.files_returned,
        "files_total": sample.files_total,
        "files_truncated": sample.files_truncated,
        "threads_returned": sample.threads_returned,
        "threads_total": sample.threads_total,
        "threads_truncated": sample.threads_truncated,
        "graphql_errors": sample.graphql_errors,
    });
    writeln!(log, "{line}")?;
    Ok(())
}
