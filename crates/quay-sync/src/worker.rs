use quay_core::{Mutation, MutationKind};
use quay_forge::{ForgeError, MutationTarget, RateGovernor, build_mutation_request, forge_message};
use quay_store::Store;

use crate::error::SyncError;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct MutationReport {
    pub sent: usize,
    pub rolled_back: usize,
    pub deferred: usize,
    pub failures: Vec<String>,
}

enum Settlement {
    Sent,
    RolledBack(String),
    Deferred(String),
}

pub async fn drain(
    store: &mut Store,
    governor: &RateGovernor,
    api_base: &str,
    limit: usize,
) -> Result<MutationReport, SyncError> {
    let mut report = MutationReport::default();
    let mut handled: Vec<i64> = Vec::new();
    for _ in 0..limit {
        let Some(mutation) = store.claim_next_mutation()? else {
            break;
        };
        if handled.contains(&mutation.id) {
            store.park_mutation(mutation.id)?;
            break;
        }
        handled.push(mutation.id);
        match settle(store, governor, api_base, &mutation).await {
            Settlement::Sent => {
                store.settle_mutation(mutation.id)?;
                report.sent += 1;
            }
            Settlement::RolledBack(reason) => {
                store.roll_back_mutation(mutation.id, &reason)?;
                report.rolled_back += 1;
                report
                    .failures
                    .push(format!("{}: {reason}", mutation.target));
            }
            Settlement::Deferred(reason) => {
                store.defer_mutation(mutation.id, &reason)?;
                report.deferred += 1;
            }
        }
    }
    Ok(report)
}

async fn settle(
    store: &Store,
    governor: &RateGovernor,
    api_base: &str,
    mutation: &Mutation,
) -> Settlement {
    let target = match locate(store, mutation) {
        Ok(Some(target)) => target,
        Ok(None) => {
            return Settlement::RolledBack(format!(
                "no local record carries the node id {}",
                mutation.target
            ));
        }
        Err(error) => return Settlement::Deferred(error.to_string()),
    };

    let request = match build_mutation_request(api_base, mutation.kind, &target, &mutation.payload)
    {
        Ok(request) => request,
        Err(error) => return Settlement::RolledBack(error.to_string()),
    };

    match governor.send(request).await {
        Ok(response) if (200..300).contains(&response.status) => Settlement::Sent,
        Ok(response) if is_definitive(response.status) => Settlement::RolledBack(format!(
            "{} {}",
            response.status,
            forge_message(&response.body)
        )),
        Ok(response) => Settlement::Deferred(format!("{} from the forge", response.status)),
        Err(error) if keeps_the_user_intent(&error) => Settlement::Deferred(error.to_string()),
        Err(error) => Settlement::RolledBack(error.to_string()),
    }
}

fn locate(store: &Store, mutation: &Mutation) -> Result<Option<MutationTarget>, SyncError> {
    let located = match mutation.kind {
        MutationKind::ResolveThread | MutationKind::UnresolveThread => {
            store.locate_thread_pull_request(&mutation.target)?
        }
        _ => store.locate_pull_request(&mutation.target)?,
    };
    Ok(located.map(|(owner, name, number)| MutationTarget {
        owner,
        name,
        number,
        node_id: mutation.target.clone(),
    }))
}

fn is_definitive(status: u16) -> bool {
    matches!(status, 400 | 401 | 403 | 404 | 409 | 410 | 422)
}

fn keeps_the_user_intent(error: &ForgeError) -> bool {
    matches!(
        error,
        ForgeError::ReadOnlyModeRejected { .. }
            | ForgeError::WriteAllowlistRejected { .. }
            | ForgeError::WriteTargetMissing
            | ForgeError::BudgetExhausted { .. }
            | ForgeError::SpeculationBudgetExhausted { .. }
            | ForgeError::Throttled { .. }
            | ForgeError::Transport(_)
    )
}
