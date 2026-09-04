use std::collections::BTreeMap;
use std::error::Error;
use std::io::Write;
use std::time::Instant;

use quay_core::Priority;
use quay_forge::{
    CacheValidators, Capabilities, Capability, DevLocks, ForgeResponse, GovernorConfig,
    OutboundRequest, RateGovernor, SingleSignOn, Token, forge_message, read_identity,
};
use serde_json::{Value, json};

use crate::paths;

const API: &str = "https://api.github.com";
const REVALIDATIONS: usize = 10;
const PLAIN_REQUESTS: usize = 10;

pub struct Observation {
    pub step: &'static str,
    pub detail: Value,
}

pub async fn measure(started_at: &str) -> Result<Vec<Observation>, Box<dyn Error>> {
    let token = Token::from_env("QUAY_TEST_TOKEN")
        .ok_or("QUAY_TEST_TOKEN is not set; M0-1 cannot be measured")?;
    let token_kind = token.kind();
    let governor = RateGovernor::new(GovernorConfig::default(), Some(token), DevLocks::from_env())?;

    std::fs::create_dir_all(paths::raw())?;
    let file_name = format!("m0-1-{}.jsonl", started_at.replace(':', ""));
    let mut log = std::fs::File::create(paths::raw().join(&file_name))?;
    let mut observations = Vec::new();

    let user = get(&governor, "/user", None).await?;
    let organizations = get(&governor, "/user/orgs", None).await?;
    let identity = read_identity(token_kind, &user, &organizations)?;
    let mut capabilities = Capabilities::from_identity(&identity);

    observations.push(Observation {
        step: "identity",
        detail: json!({
            "token_kind": token_kind.label(),
            "login_resolved": !identity.login.is_empty(),
            "organization_count": identity.organizations.len(),
            "granted_scopes": identity.scopes.listed(),
            "single_sign_on": match &identity.single_sign_on {
                SingleSignOn::NoRestriction => "none".to_owned(),
                SingleSignOn::PartialResults { organization_ids } =>
                    format!("partial-results, {} organization(s) hidden", organization_ids.len()),
                SingleSignOn::AuthorizationRequired { .. } => "authorization required".to_owned(),
            },
        }),
    });

    let probe = get(&governor, "/notifications", None).await?;
    capabilities.observe_probe(
        Capability::Notifications,
        probe.status,
        &forge_message(&probe.body),
    );
    let validators = probe.validators.clone();
    observations.push(Observation {
        step: "notifications_probe",
        detail: json!({
            "status": probe.status,
            "capability": capabilities.get(Capability::Notifications).is_enabled(),
            "message": if probe.status == 200 { String::new() } else { forge_message(&probe.body) },
            "poll_interval_seconds": probe.poll_interval.map(|interval| interval.as_secs()),
            "etag_offered": validators.as_ref().is_some_and(|v| v.etag.is_some()),
            "last_modified_offered": validators.as_ref().is_some_and(|v| v.last_modified.is_some()),
            "bytes": probe.bytes,
            "entries": serde_json::from_str::<Vec<Value>>(&probe.body)
                .map(|entries| entries.len())
                .unwrap_or(0),
        }),
    });

    if probe.status == 200 {
        observations
            .push(campaign(&governor, "revalidated", REVALIDATIONS, validators.clone()).await?);
        observations.push(campaign(&governor, "plain", PLAIN_REQUESTS, None).await?);
    }

    for observation in &observations {
        writeln!(
            log,
            "{}",
            json!({
                "scenario": "m0-1",
                "run_started_at": started_at,
                "machine": crate::machine(),
                "step": observation.step,
                "detail": observation.detail,
            })
        )?;
        println!("m0-1: {} {}", observation.step, observation.detail);
    }
    Ok(observations)
}

async fn campaign(
    governor: &RateGovernor,
    label: &'static str,
    repetitions: usize,
    validators: Option<CacheValidators>,
) -> Result<Observation, Box<dyn Error>> {
    let before = rate_limit_resources(governor).await?;
    let mut statuses = Vec::new();
    let mut durations = Vec::new();
    let mut header_remaining = Vec::new();
    let mut bytes = 0usize;

    for _ in 0..repetitions {
        let started = Instant::now();
        let response = get(governor, "/notifications", validators.clone()).await?;
        durations.push(started.elapsed().as_secs_f64() * 1_000.0);
        statuses.push(response.status);
        header_remaining.push(response.rate_limit.map(|snapshot| snapshot.remaining));
        bytes += response.bytes;
    }

    let after = rate_limit_resources(governor).await?;
    Ok(Observation {
        step: if label == "revalidated" {
            "revalidated_campaign"
        } else {
            "plain_campaign"
        },
        detail: json!({
            "requests": repetitions,
            "statuses": statuses,

            "rate_limit_endpoint_buckets_that_moved": buckets_that_moved(&before, &after),
            "quota_remaining_in_response_headers": header_remaining,
            "bytes": bytes,
            "elapsed_ms": crate::stats::Percentiles::of(&durations),
        }),
    })
}

async fn rate_limit_resources(
    governor: &RateGovernor,
) -> Result<BTreeMap<String, u64>, Box<dyn Error>> {
    let response = get(governor, "/rate_limit", None).await?;
    let parsed: Value = serde_json::from_str(&response.body)?;
    let resources = parsed
        .pointer("/resources")
        .and_then(Value::as_object)
        .ok_or("the rate limit endpoint did not report its resources")?;
    Ok(resources
        .iter()
        .filter_map(|(name, bucket)| {
            bucket
                .get("remaining")
                .and_then(Value::as_u64)
                .map(|remaining| (name.clone(), remaining))
        })
        .collect())
}

fn buckets_that_moved(
    before: &BTreeMap<String, u64>,
    after: &BTreeMap<String, u64>,
) -> BTreeMap<String, i64> {
    before
        .iter()
        .filter_map(|(name, start)| {
            let end = after.get(name)?;
            let delta = *start as i64 - *end as i64;
            if delta == 0 {
                None
            } else {
                Some((name.clone(), delta))
            }
        })
        .collect()
}

async fn get(
    governor: &RateGovernor,
    path: &str,
    validators: Option<CacheValidators>,
) -> Result<ForgeResponse, Box<dyn Error>> {
    let request =
        OutboundRequest::rest_read(format!("{API}{path}"), Priority::User).revalidating(validators);
    Ok(governor.send(request).await?)
}

pub fn summary(observations: &[Observation], started_at: &str, raw_log: &str) -> Value {
    json!({
        "scenario": "m0-1",
        "measured_at": started_at,
        "machine": crate::machine(),
        "raw_log": raw_log,
        "rows": observations
            .iter()
            .map(|observation| json!({
                "step": observation.step,
                "detail": observation.detail,
            }))
            .collect::<Vec<Value>>(),
    })
}
