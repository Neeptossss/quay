use std::error::Error;
use std::fmt::Write as _;
use std::path::PathBuf;

use serde_json::Value;

use crate::budgets::{self, Bound};
use crate::measure;
use crate::paths;

pub fn generate() -> Result<PathBuf, Box<dyn Error>> {
    let mut page = String::new();
    writeln!(page, "# Rapport de mesures — Quay\n")?;
    writeln!(
        page,
        "Ce fichier est généré par `cargo run -p xtask -- report` à partir des logs bruts de \
         `measurements/raw/`. Il ne se modifie pas à la main.\n"
    )?;
    writeln!(page, "- Généré le : {}", measure::now_utc())?;
    writeln!(page, "- Machine : {}\n", crate::machine())?;

    write_budgets(&mut page)?;
    write_j1a(&mut page)?;
    write_j1b(&mut page)?;

    let path = paths::report();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, page)?;
    Ok(path)
}

fn write_budgets(page: &mut String) -> Result<(), Box<dyn Error>> {
    writeln!(page, "## Budgets du §4\n")?;
    writeln!(
        page,
        "`MISSING` signifie qu'aucune mesure n'existe encore pour ce budget : il est rouge parce \
         qu'un budget non mesuré est un budget non tenu, pas parce qu'il est dépassé.\n"
    )?;
    writeln!(
        page,
        "| Budget | Réf. | Borne | Statistique | Instrument | Mesure | Verdict | Log brut |"
    )?;
    writeln!(page, "|---|---|---|---|---|---|---|---|")?;
    for status in budgets::status() {
        let bound = match status.budget.bound {
            Bound::AtMost => "≤",
            Bound::AtLeast => "≥",
        };
        let (measured, raw_log) = match status.summary.as_ref() {
            Some(summary) => (
                format!("{:.3} {}", summary.value, summary.unit),
                format!("`{}`", summary.raw_log),
            ),
            None => ("—".to_owned(), "—".to_owned()),
        };
        writeln!(
            page,
            "| `{}` ({}) | {} | {} {} {} | {} | {} | {} | **{}** | {} |",
            status.budget.id,
            status.budget.metric,
            status.budget.specification,
            bound,
            status.budget.limit,
            status.budget.unit,
            status.budget.statistic,
            status.budget.instrument,
            measured,
            status.verdict.label(),
            raw_log
        )?;
    }
    writeln!(page)?;
    Ok(())
}

fn write_j1a(page: &mut String) -> Result<(), Box<dyn Error>> {
    writeln!(
        page,
        "## J1-a — point de rupture de la requête GraphQL de détail de pull request\n"
    )?;
    let Some(summary) = load("j1a") else {
        writeln!(
            page,
            "Pas encore mesuré. Lancer `cargo run -p xtask -- measure j1a`.\n"
        )?;
        return Ok(());
    };

    writeln!(
        page,
        "- Mesuré le : {}",
        text(&summary, "/measured_at", "inconnu")
    )?;
    writeln!(
        page,
        "- Machine : {}",
        text(&summary, "/machine", "inconnue")
    )?;
    writeln!(
        page,
        "- Log brut : `{}`\n",
        text(&summary, "/raw_log", "inconnu")
    )?;

    writeln!(
        page,
        "| Cible | Sélection | Fichiers | Threads | Mode | Pagination | Rép. | p50 ms | p95 ms | \
         max ms | Octets | Coût | Nœuds | Fichiers rendus | Threads rendus | Tronqué | Erreurs |"
    )?;
    writeln!(
        page,
        "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|"
    )?;
    let detail_rows = rows(&summary);
    for row in &detail_rows {
        let errors = row
            .get("graphql_errors")
            .and_then(Value::as_array)
            .map(|errors| errors.len())
            .unwrap_or(0);
        let truncated = row
            .get("files_truncated")
            .and_then(Value::as_bool)
            .unwrap_or(false)
            || row
                .get("threads_truncated")
                .and_then(Value::as_bool)
                .unwrap_or(false);
        writeln!(
            page,
            "| `{}` | {} | {} | {} | {} | {} | {} | {:.0} | {:.0} | {:.0} | {} | {} | {} | {}/{} | {}/{} | {} | {} |",
            text(row, "/target", "?"),
            text(row, "/selection", "?"),
            number(row, "/changed_files"),
            number(row, "/review_threads"),
            text(row, "/mode", "?"),
            text(row, "/configuration", "?"),
            number(row, "/repetitions"),
            float(row, "/elapsed_ms/p50"),
            float(row, "/elapsed_ms/p95"),
            float(row, "/elapsed_ms/max"),
            number(row, "/bytes"),
            number(row, "/graphql_cost"),
            number(row, "/graphql_node_count"),
            number(row, "/files_returned"),
            number(row, "/files_total"),
            number(row, "/threads_returned"),
            number(row, "/threads_total"),
            if truncated { "oui" } else { "non" },
            errors
        )?;
    }
    writeln!(page)?;

    let messages: Vec<String> = detail_rows
        .iter()
        .flat_map(|row| {
            row.get("graphql_errors")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|message| message.as_str().map(str::to_owned))
        .collect();
    if !messages.is_empty() {
        writeln!(page, "### Erreurs GraphQL rencontrées\n")?;
        let mut unique = messages;
        unique.sort();
        unique.dedup();
        for message in unique {
            writeln!(page, "- {message}")?;
        }
        writeln!(page)?;
    }
    Ok(())
}

fn write_j1b(page: &mut String) -> Result<(), Box<dyn Error>> {
    writeln!(page, "## J1-b — requête d'inbox sur dataset synthétique\n")?;
    let Some(summary) = load("j1b") else {
        writeln!(
            page,
            "Pas encore mesuré. Lancer `cargo run -p xtask -- measure j1b`.\n"
        )?;
        return Ok(());
    };

    writeln!(
        page,
        "- Mesuré le : {}",
        text(&summary, "/measured_at", "inconnu")
    )?;
    writeln!(
        page,
        "- Machine : {}\n",
        text(&summary, "/machine", "inconnue")
    )?;
    writeln!(
        page,
        "Le budget `inbox_query` retient le p99 le plus défavorable des quatre requêtes sur le \
         dataset de référence, cache chaud.\n"
    )?;

    writeln!(
        page,
        "| Dataset | Requête | Cache | Lignes | p50 ms | p95 ms | p99 ms | max ms | Échantillons \
         | Log brut |"
    )?;
    writeln!(page, "|---|---|---|---|---|---|---|---|---|---|")?;
    let query_rows = rows(&summary);
    for row in &query_rows {
        writeln!(
            page,
            "| {} | `{}` | {} | {} | {:.3} | {:.3} | {:.3} | {:.3} | {} | `{}` |",
            text(row, "/dataset", "?"),
            text(row, "/query", "?"),
            text(row, "/cache", "?"),
            number(row, "/rows"),
            float(row, "/percentiles/p50"),
            float(row, "/percentiles/p95"),
            float(row, "/percentiles/p99"),
            float(row, "/percentiles/max"),
            number(row, "/percentiles/samples"),
            text(row, "/raw_log", "?")
        )?;
    }
    writeln!(page)?;

    writeln!(page, "### Plans d'exécution SQLite\n")?;
    let mut seen: Vec<String> = Vec::new();
    for row in &query_rows {
        let query = text(row, "/query", "?").to_owned();
        if seen.contains(&query) {
            continue;
        }
        seen.push(query.clone());
        writeln!(page, "- `{}` : {}", query, text(row, "/plan", "?"))?;
    }
    writeln!(page)?;
    Ok(())
}

fn load(scenario: &str) -> Option<Value> {
    let path = paths::summaries().join(format!("{scenario}.json"));
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

fn rows(summary: &Value) -> Vec<Value> {
    summary
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

fn text<'a>(value: &'a Value, pointer: &str, fallback: &'a str) -> &'a str {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or(fallback)
}

fn number(value: &Value, pointer: &str) -> u64 {
    value.pointer(pointer).and_then(Value::as_u64).unwrap_or(0)
}

fn float(value: &Value, pointer: &str) -> f64 {
    value
        .pointer(pointer)
        .and_then(Value::as_f64)
        .unwrap_or(0.0)
}
