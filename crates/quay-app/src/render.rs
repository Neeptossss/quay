use quay_store::inbox::InboxRow;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const TITLE_WIDTH: usize = 52;
const REPOSITORY_WIDTH: usize = 26;
const AUTHOR_WIDTH: usize = 14;

pub struct Palette {
    enabled: bool,
}

impl Palette {
    pub fn from_env() -> Self {
        Self {
            enabled: std::env::var("NO_COLOR").is_err(),
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.enabled {
            format!("\u{1b}[{code}m{text}\u{1b}[0m")
        } else {
            text.to_owned()
        }
    }

    fn checks(&self, state: Option<&str>) -> String {
        let (code, glyph) = match state {
            Some("success") => ("32", "ok "),
            Some("failure") | Some("error") => ("31", "err"),
            Some("pending") | Some("expected") => ("33", "run"),
            _ => ("90", "  -"),
        };
        self.paint(code, glyph)
    }

    fn review(&self, state: Option<&str>) -> String {
        let (code, glyph) = match state {
            Some("approved") => ("32", "appr"),
            Some("changes_requested") => ("31", "chng"),
            Some("review_required") => ("33", "reqd"),
            _ => ("90", "   -"),
        };
        self.paint(code, glyph)
    }
}

pub fn header() -> String {
    format!(
        "{:<repository$}  {:>7}  {:<3}  {:<4}  {:>3}  {:<title$}  {:<author$}  {}",
        "REPOSITORY",
        "NUMBER",
        "CI",
        "REV",
        "THR",
        "TITLE",
        "AUTHOR",
        "UPDATED",
        repository = REPOSITORY_WIDTH,
        title = TITLE_WIDTH,
        author = AUTHOR_WIDTH,
    )
}

pub fn row(row: &InboxRow, palette: &Palette, now: OffsetDateTime) -> String {
    let repository = clip(&format!("{}/{}", row.owner, row.name), REPOSITORY_WIDTH);
    let title = clip(&row.title, TITLE_WIDTH);
    let author = clip(&row.author, AUTHOR_WIDTH);
    let draft = if row.is_draft { "~" } else { " " };
    format!(
        "{:<repository$}  {:>6}{}  {}  {}  {:>3}  {:<title$}  {:<author$}  {}",
        repository,
        row.number,
        draft,
        palette.checks(row.checks_state.as_deref()),
        palette.review(row.review_state.as_deref()),
        row.unresolved_threads,
        title,
        author,
        age(&row.updated_at, now),
        repository = REPOSITORY_WIDTH,
        title = TITLE_WIDTH,
        author = AUTHOR_WIDTH,
    )
}

pub fn empty_inbox_invitation() -> String {
    "Aucune revue en attente. `quay sync` pour rafraîchir.".to_owned()
}

fn clip(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_owned();
    }
    let kept: String = text.chars().take(width.saturating_sub(1)).collect();
    format!("{kept}…")
}

pub fn age(updated_at: &str, now: OffsetDateTime) -> String {
    let Ok(moment) = OffsetDateTime::parse(updated_at, &Rfc3339) else {
        return "?".to_owned();
    };
    let seconds = (now - moment).whole_seconds().max(0);
    match seconds {
        0..=89 => format!("{seconds}s"),
        90..=5399 => format!("{}m", seconds / 60),
        5400..=172_799 => format!("{}h", seconds / 3_600),
        _ => format!("{}d", seconds / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use super::{age, clip};

    fn now() -> OffsetDateTime {
        OffsetDateTime::from_unix_timestamp(1_788_516_000).unwrap_or(OffsetDateTime::UNIX_EPOCH)
    }

    #[test]
    fn a_short_text_is_left_untouched() {
        assert_eq!(clip("api", 10), "api");
    }

    #[test]
    fn a_long_text_is_clipped_with_an_ellipsis_and_never_exceeds_the_width() {
        let clipped = clip("a-very-long-repository-name", 10);
        assert_eq!(clipped.chars().count(), 10);
        assert!(clipped.ends_with('…'));
    }

    #[test]
    fn a_multibyte_title_is_clipped_on_characters_not_on_bytes() {
        let clipped = clip("réécriture du moteur de synchronisation", 8);
        assert_eq!(clipped.chars().count(), 8);
    }

    #[test]
    fn an_age_is_rendered_in_the_largest_unit_that_stays_readable() {
        assert_eq!(age("2026-09-04T09:59:30Z", now()), "30s");
        assert_eq!(age("2026-09-04T09:30:00Z", now()), "30m");
        assert_eq!(age("2026-09-04T04:00:00Z", now()), "6h");
        assert_eq!(age("2026-09-01T10:00:00Z", now()), "3d");
    }

    #[test]
    fn an_unparseable_instant_renders_as_unknown_rather_than_as_now() {
        assert_eq!(age("not a date", now()), "?");
    }

    #[test]
    fn an_instant_in_the_future_never_renders_as_negative() {
        assert_eq!(age("2026-09-05T10:00:00Z", now()), "0s");
    }
}
