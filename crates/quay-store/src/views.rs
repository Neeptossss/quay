use rusqlite::{Connection, Row, params};
use serde::{Deserialize, Serialize};

use crate::error::StoreError;

pub const SHIPPED: &str = include_str!("../views/default.json");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedView {
    pub name: String,
    pub query: String,
    pub columns: Vec<String>,
    pub sort: String,
    pub position: i64,
    pub shortcut: Option<String>,
}

pub fn shipped() -> Result<Vec<SavedView>, StoreError> {
    serde_json::from_str(SHIPPED).map_err(|error| StoreError::UnreadableValue {
        column: "views/default.json",
        value: error.to_string(),
    })
}

fn read(row: &Row<'_>) -> Result<SavedView, rusqlite::Error> {
    let columns: String = row.get(1)?;
    Ok(SavedView {
        name: row.get(0)?,
        columns: columns
            .split(',')
            .filter(|column| !column.is_empty())
            .map(str::to_owned)
            .collect(),
        query: row.get(2)?,
        sort: row.get(3)?,
        position: row.get(4)?,
        shortcut: row.get(5)?,
    })
}

pub fn all(connection: &Connection) -> Result<Vec<SavedView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT name, columns, query, sort, position, shortcut FROM saved_view
         ORDER BY position, name",
    )?;
    Ok(statement
        .query_map([], read)?
        .collect::<Result<Vec<SavedView>, rusqlite::Error>>()?)
}

pub fn by_name(connection: &Connection, name: &str) -> Result<Option<SavedView>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT name, columns, query, sort, position, shortcut FROM saved_view WHERE name = ?1",
    )?;
    let mut rows = statement.query_map((name,), read)?;
    match rows.next() {
        None => Ok(None),
        Some(view) => Ok(Some(view?)),
    }
}

pub fn save(connection: &Connection, view: &SavedView) -> Result<(), StoreError> {
    connection.execute(
        "INSERT INTO saved_view (name, query, columns, sort, position, shortcut)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            view.name,
            view.query,
            view.columns.join(","),
            view.sort,
            view.position,
            view.shortcut,
        ],
    )?;
    Ok(())
}

pub fn remove(connection: &Connection, name: &str) -> Result<bool, StoreError> {
    Ok(connection.execute("DELETE FROM saved_view WHERE name = ?1", (name,))? == 1)
}

pub fn install_shipped_if_empty(connection: &Connection) -> Result<usize, StoreError> {
    let existing: i64 =
        connection.query_row("SELECT COUNT(*) FROM saved_view", [], |row| row.get(0))?;
    if existing > 0 {
        return Ok(0);
    }
    let views = shipped()?;
    for view in &views {
        save(connection, view)?;
    }
    Ok(views.len())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use quay_core::parse_query;

    use super::{SavedView, all, by_name, install_shipped_if_empty, remove, save, shipped};
    use crate::compile::compile;
    use crate::{Store, StoreError};

    fn store() -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("quay.db")).unwrap();
        (directory, store)
    }

    #[test]
    fn every_shipped_view_parses_and_compiles_so_none_is_discoverable_yet_broken() {
        for view in shipped().unwrap() {
            let parsed = match parse_query(&view.query) {
                Ok(parsed) => parsed,
                Err(error) => panic!("the view {:?} does not parse: {error}", view.name),
            };
            if let Err(error) = compile(&parsed, "octocat", 50) {
                panic!("the view {:?} does not compile: {error}", view.name);
            }
        }
    }

    #[test]
    fn no_two_shipped_views_claim_the_same_shortcut() {
        let mut shortcuts: Vec<String> = shipped()
            .unwrap()
            .into_iter()
            .filter_map(|view| view.shortcut)
            .collect();
        let claimed = shortcuts.len();
        shortcuts.sort();
        shortcuts.dedup();
        assert_eq!(shortcuts.len(), claimed);
    }

    #[test]
    fn every_shipped_view_names_the_columns_it_shows() {
        for view in shipped().unwrap() {
            assert!(!view.columns.is_empty(), "{}", view.name);
            assert!(view.columns.contains(&"title".to_owned()), "{}", view.name);
        }
    }

    #[test]
    fn the_shipped_views_are_installed_once_and_never_again() {
        let (_directory, store) = store();
        assert_eq!(
            install_shipped_if_empty(store.connection()).unwrap(),
            shipped().unwrap().len()
        );
        assert_eq!(install_shipped_if_empty(store.connection()).unwrap(), 0);
        assert_eq!(
            all(store.connection()).unwrap().len(),
            shipped().unwrap().len()
        );
    }

    #[test]
    fn views_come_back_in_the_order_their_position_declares() {
        let (_directory, store) = store();
        install_shipped_if_empty(store.connection()).unwrap();
        let positions: Vec<i64> = all(store.connection())
            .unwrap()
            .into_iter()
            .map(|view| view.position)
            .collect();
        let mut sorted = positions.clone();
        sorted.sort_unstable();
        assert_eq!(positions, sorted);
    }

    #[test]
    fn a_view_survives_a_round_trip_through_the_database() {
        let (_directory, store) = store();
        let view = SavedView {
            name: "Bloqué".to_owned(),
            query: "is:pr is:open checks:failing".to_owned(),
            columns: vec!["repo".to_owned(), "title".to_owned()],
            sort: "updated-desc".to_owned(),
            position: 9,
            shortcut: Some("g x".to_owned()),
        };
        save(store.connection(), &view).unwrap();
        assert_eq!(by_name(store.connection(), "Bloqué").unwrap(), Some(view));
    }

    #[test]
    fn saving_two_views_under_the_same_name_is_refused_rather_than_shadowing_one() {
        let (_directory, store) = store();
        install_shipped_if_empty(store.connection()).unwrap();
        let existing = by_name(store.connection(), "view.to_review")
            .unwrap()
            .unwrap();
        assert!(matches!(
            save(store.connection(), &existing),
            Err(StoreError::Sqlite(_))
        ));
    }

    #[test]
    fn an_unknown_view_reads_as_absent_and_removing_it_reports_that_it_was_absent() {
        let (_directory, store) = store();
        assert_eq!(by_name(store.connection(), "Néant").unwrap(), None);
        assert!(!remove(store.connection(), "Néant").unwrap());
    }

    #[test]
    fn removing_a_view_takes_it_out_of_the_listing() {
        let (_directory, store) = store();
        install_shipped_if_empty(store.connection()).unwrap();
        assert!(remove(store.connection(), "view.drafts").unwrap());
        assert!(
            !all(store.connection())
                .unwrap()
                .iter()
                .any(|view| view.name == "view.drafts")
        );
    }
}
