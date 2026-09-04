use quay_core::{ChangeSignal, EntityId, Timestamp};

use crate::transport::NotificationPayload;

pub fn pull_request_locator(subject_url: &str) -> Option<(String, String, i64)> {
    let tail = subject_url.split("/repos/").nth(1)?;
    let mut parts = tail.split('/');
    let owner = parts.next()?;
    let name = parts.next()?;
    let collection = parts.next()?;
    if collection != "pulls" {
        return None;
    }
    let number = parts.next()?.parse().ok()?;
    if owner.is_empty() || name.is_empty() {
        return None;
    }
    Some((owner.to_owned(), name.to_owned(), number))
}

pub fn signal_from(payload: &NotificationPayload, updated_at: Timestamp) -> Option<ChangeSignal> {
    if payload.subject.kind != "PullRequest" {
        return None;
    }
    let url = payload.subject.url.as_deref()?;
    let (owner, name, number) = pull_request_locator(url)?;
    Some(ChangeSignal::new(
        EntityId::pull_request(&owner, &name, number),
        updated_at,
    ))
}

#[cfg(test)]
mod tests {
    use quay_core::Timestamp;

    use super::{pull_request_locator, signal_from};
    use crate::transport::{
        NotificationPayload, NotificationRepositoryPayload, NotificationSubjectPayload,
    };

    fn payload(kind: &str, url: Option<&str>) -> NotificationPayload {
        NotificationPayload {
            id: "1".to_owned(),
            unread: true,
            updated_at: "2026-09-04T10:00:00Z".to_owned(),
            reason: "review_requested".to_owned(),
            subject: NotificationSubjectPayload {
                title: "Fix the thing".to_owned(),
                url: url.map(str::to_owned),
                kind: kind.to_owned(),
            },
            repository: NotificationRepositoryPayload {
                full_name: "acme/api".to_owned(),
                node_id: "R_1".to_owned(),
            },
        }
    }

    #[test]
    fn a_pull_request_subject_url_yields_the_owner_the_name_and_the_number() {
        assert_eq!(
            pull_request_locator("https://api.github.com/repos/acme/api/pulls/1234"),
            Some(("acme".to_owned(), "api".to_owned(), 1234))
        );
    }

    #[test]
    fn an_enterprise_base_url_is_parsed_the_same_way() {
        assert_eq!(
            pull_request_locator("https://github.example.com/api/v3/repos/acme/api/pulls/7"),
            Some(("acme".to_owned(), "api".to_owned(), 7))
        );
    }

    #[test]
    fn an_issue_subject_url_is_not_mistaken_for_a_pull_request() {
        assert!(pull_request_locator("https://api.github.com/repos/acme/api/issues/12").is_none());
    }

    #[test]
    fn a_malformed_subject_url_yields_no_locator_rather_than_a_wrong_one() {
        for url in [
            "https://api.github.com/repos/acme/api/pulls",
            "https://api.github.com/repos/acme/api/pulls/not-a-number",
            "https://api.github.com/notifications",
            "",
        ] {
            assert!(pull_request_locator(url).is_none(), "{url}");
        }
    }

    #[test]
    fn a_notification_about_an_issue_produces_no_signal() {
        assert!(
            signal_from(
                &payload(
                    "Issue",
                    Some("https://api.github.com/repos/acme/api/issues/12")
                ),
                Timestamp(0)
            )
            .is_none()
        );
    }

    #[test]
    fn a_notification_without_a_subject_url_produces_no_signal() {
        assert!(signal_from(&payload("PullRequest", None), Timestamp(0)).is_none());
    }

    #[test]
    fn a_pull_request_notification_produces_a_signal_carrying_its_locator() {
        let signal = signal_from(
            &payload(
                "PullRequest",
                Some("https://api.github.com/repos/acme/api/pulls/1234"),
            ),
            Timestamp(1_788_000_000),
        );
        match signal {
            Some(signal) => {
                assert_eq!(signal.entity.key, "acme/api#1234");
                assert_eq!(signal.updated_at, Timestamp(1_788_000_000));
            }
            None => panic!("a pull request notification must produce a signal"),
        }
    }
}
