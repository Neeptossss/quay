use rusqlite::{Connection, params};

use crate::error::StoreError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    pub login: String,
    pub display_name: Option<String>,
    pub open_pull_requests: i64,
}

pub fn replace(
    connection: &mut Connection,
    account_id: i64,
    memberships: &[(String, Option<String>)],
) -> Result<(), StoreError> {
    let transaction = connection.transaction()?;
    transaction.execute(
        "DELETE FROM organization WHERE account_id = ?1",
        params![account_id],
    )?;
    {
        let mut insert = transaction.prepare(
            "INSERT INTO organization (account_id, login, display_name, is_member)
             VALUES (?1, ?2, ?3, 1)",
        )?;
        for (login, display_name) in memberships {
            insert.execute(params![account_id, login, display_name])?;
        }
    }
    transaction.commit()?;
    Ok(())
}

pub fn all(connection: &Connection, account_id: i64) -> Result<Vec<Organization>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT o.login, o.display_name,
                (SELECT COUNT(*) FROM pull_request pr
                  JOIN repo r ON r.id = pr.repo_id
                 WHERE r.owner = o.login AND pr.state = 'open' AND r.is_tracked = 1)
         FROM organization o
         WHERE o.account_id = ?1
         ORDER BY o.login",
    )?;
    Ok(statement
        .query_map((account_id,), |row| {
            Ok(Organization {
                login: row.get(0)?,
                display_name: row.get(1)?,
                open_pull_requests: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<Organization>, rusqlite::Error>>()?)
}

pub fn owners_with_pull_requests(connection: &Connection) -> Result<Vec<Organization>, StoreError> {
    let mut statement = connection.prepare_cached(
        "SELECT r.owner, NULL, COUNT(*) FROM pull_request pr
         JOIN repo r ON r.id = pr.repo_id
         WHERE pr.state = 'open' AND r.is_tracked = 1
         GROUP BY r.owner ORDER BY r.owner",
    )?;
    Ok(statement
        .query_map([], |row| {
            Ok(Organization {
                login: row.get(0)?,
                display_name: row.get(1)?,
                open_pull_requests: row.get(2)?,
            })
        })?
        .collect::<Result<Vec<Organization>, rusqlite::Error>>()?)
}

pub fn selected(connection: &Connection, account_id: i64) -> Result<Option<String>, StoreError> {
    let mut statement =
        connection.prepare_cached("SELECT selected_organization FROM account WHERE id = ?1")?;
    let mut rows = statement.query_map((account_id,), |row| row.get::<_, Option<String>>(0))?;
    match rows.next() {
        None => Ok(None),
        Some(value) => Ok(value?),
    }
}

pub fn select(
    connection: &Connection,
    account_id: i64,
    login: Option<&str>,
) -> Result<(), StoreError> {
    connection.execute(
        "UPDATE account SET selected_organization = ?2 WHERE id = ?1",
        params![account_id, login],
    )?;
    Ok(())
}

pub fn scoped(query: &str, login: Option<&str>) -> String {
    let stripped: Vec<&str> = query
        .split_whitespace()
        .filter(|token| !token.starts_with("org:") && !token.starts_with("-org:"))
        .collect();
    match login {
        None => stripped.join(" "),
        Some(login) => {
            let mut tokens = stripped;
            tokens.push("");
            let mut rendered = tokens.join(" ");
            rendered.push_str(&format!("org:{login}"));
            rendered.trim().to_owned()
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{all, replace, scoped, select, selected};
    use crate::Store;

    fn store() -> (tempfile::TempDir, Store) {
        let directory = tempfile::tempdir().unwrap();
        let store = Store::open(&directory.path().join("quay.db")).unwrap();
        store
            .connection()
            .execute_batch(
                "INSERT INTO account (id, host, login, auth_kind, keychain_ref)
                     VALUES (1, 'api.github.com', 'octocat', 'pat_classic', 'ref');
                 INSERT INTO repo (id, account_id, node_id, owner, name, is_tracked)
                     VALUES (1, 1, 'R_1', 'acme', 'api', 1);
                 INSERT INTO pull_request (
                     id, repo_id, number, node_id, title, state, is_draft, author,
                     base_ref, head_sha, updated_at)
                     VALUES (1, 1, 7, 'PR_1', 'Fix', 'open', 0, 'avery', 'main', 'sha', '2026-09-04T10:00:00Z');",
            )
            .unwrap();
        (directory, store)
    }

    #[test]
    fn an_account_starts_with_no_organisation_and_no_selection() {
        let (_directory, store) = store();
        assert!(all(store.connection(), 1).unwrap().is_empty());
        assert_eq!(selected(store.connection(), 1).unwrap(), None);
    }

    #[test]
    fn organisations_come_back_with_the_count_of_open_pull_requests() {
        let (_directory, mut store) = store();
        let memberships = vec![
            ("acme".to_owned(), Some("Acme".to_owned())),
            ("contoso".to_owned(), None),
        ];
        replace(store.connection_mut(), 1, &memberships).unwrap();
        let listed = all(store.connection(), 1).unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(listed[0].login, "acme");
        assert_eq!(listed[0].open_pull_requests, 1);
        assert_eq!(listed[1].open_pull_requests, 0);
    }

    #[test]
    fn replacing_the_list_forgets_the_organisations_the_user_left() {
        let (_directory, mut store) = store();
        replace(store.connection_mut(), 1, &[("acme".to_owned(), None)]).unwrap();
        replace(store.connection_mut(), 1, &[("contoso".to_owned(), None)]).unwrap();
        let listed = all(store.connection(), 1).unwrap();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].login, "contoso");
    }

    #[test]
    fn a_selection_survives_and_can_be_cleared() {
        let (_directory, store) = store();
        select(store.connection(), 1, Some("acme")).unwrap();
        assert_eq!(
            selected(store.connection(), 1).unwrap().as_deref(),
            Some("acme")
        );
        select(store.connection(), 1, None).unwrap();
        assert_eq!(selected(store.connection(), 1).unwrap(), None);
    }

    #[test]
    fn scoping_a_query_appends_the_organisation_the_user_chose() {
        assert_eq!(
            scoped("is:pr is:open sort:updated-desc", Some("acme")),
            "is:pr is:open sort:updated-desc org:acme"
        );
    }

    #[test]
    fn scoping_replaces_an_organisation_already_in_the_query_rather_than_stacking() {
        assert_eq!(
            scoped("is:pr org:contoso is:open", Some("acme")),
            "is:pr is:open org:acme"
        );
    }

    #[test]
    fn clearing_the_scope_takes_the_organisation_back_out() {
        assert_eq!(scoped("is:pr org:acme is:open", None), "is:pr is:open");
    }

    #[test]
    fn scoping_an_empty_query_yields_only_the_organisation() {
        assert_eq!(scoped("", Some("acme")), "org:acme");
        assert_eq!(scoped("", None), "");
    }

    #[test]
    fn a_negated_organisation_already_present_is_also_replaced() {
        assert_eq!(
            scoped("is:pr -org:acme", Some("contoso")),
            "is:pr org:contoso"
        );
    }
}
