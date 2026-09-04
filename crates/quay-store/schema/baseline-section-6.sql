CREATE TABLE account (
  id            INTEGER PRIMARY KEY,
  host          TEXT NOT NULL,
  login         TEXT NOT NULL,
  auth_kind     TEXT NOT NULL,
  keychain_ref  TEXT NOT NULL,
  UNIQUE(host, login)
);

CREATE TABLE repo (
  id            INTEGER PRIMARY KEY,
  account_id    INTEGER NOT NULL REFERENCES account(id),
  node_id       TEXT NOT NULL UNIQUE,
  owner         TEXT NOT NULL,
  name          TEXT NOT NULL,
  default_branch TEXT,
  local_path    TEXT,
  is_tracked    INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE pull_request (
  id            INTEGER PRIMARY KEY,
  repo_id       INTEGER NOT NULL REFERENCES repo(id),
  number        INTEGER NOT NULL,
  node_id       TEXT NOT NULL UNIQUE,
  title         TEXT NOT NULL,
  state         TEXT NOT NULL,
  is_draft      INTEGER NOT NULL,
  author        TEXT NOT NULL,
  base_ref      TEXT NOT NULL,
  head_sha      TEXT NOT NULL,
  review_state  TEXT,
  checks_state  TEXT,
  mergeable     TEXT,
  updated_at    TEXT NOT NULL,
  raw           BLOB,
  UNIQUE(repo_id, number)
);

CREATE INDEX idx_pr_inbox ON pull_request(state, updated_at DESC);

CREATE TABLE review_thread (
  id            INTEGER PRIMARY KEY,
  pr_id         INTEGER NOT NULL REFERENCES pull_request(id) ON DELETE CASCADE,
  node_id       TEXT NOT NULL UNIQUE,
  path          TEXT NOT NULL,
  line          INTEGER,
  side          TEXT,
  original_line INTEGER,
  diff_hunk     TEXT,
  is_resolved   INTEGER NOT NULL DEFAULT 0,
  is_outdated   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE review_comment (
  id            INTEGER PRIMARY KEY,
  thread_id     INTEGER NOT NULL REFERENCES review_thread(id) ON DELETE CASCADE,
  node_id       TEXT NOT NULL UNIQUE,
  author        TEXT NOT NULL,
  body          TEXT NOT NULL,
  created_at    TEXT NOT NULL
);

CREATE TABLE resource_cache (
  key           TEXT PRIMARY KEY,
  etag          TEXT,
  last_modified TEXT,
  fetched_at    INTEGER NOT NULL,
  stale_after   INTEGER NOT NULL,
  tier          TEXT NOT NULL
);

CREATE TABLE mutation (
  id            INTEGER PRIMARY KEY,
  kind          TEXT NOT NULL,
  target        TEXT NOT NULL,
  payload       TEXT NOT NULL,
  idempotency   TEXT NOT NULL UNIQUE,
  state         TEXT NOT NULL,
  attempts      INTEGER NOT NULL DEFAULT 0,
  last_error    TEXT,
  created_at    INTEGER NOT NULL
);

CREATE TABLE saved_view (
  id            INTEGER PRIMARY KEY,
  name          TEXT NOT NULL,
  query         TEXT NOT NULL,
  columns       TEXT NOT NULL,
  sort          TEXT NOT NULL,
  position      INTEGER NOT NULL,
  shortcut      TEXT
);

CREATE TABLE navigation_event (
  id            INTEGER PRIMARY KEY,
  from_state    TEXT NOT NULL,
  to_state      TEXT NOT NULL,
  action        TEXT NOT NULL,
  entity_kind   TEXT,
  dwell_ms      INTEGER,
  at            INTEGER NOT NULL
);

CREATE INDEX idx_nav_transition ON navigation_event(from_state, action);

CREATE TABLE preload_outcome (
  id            INTEGER PRIMARY KEY,
  key           TEXT NOT NULL,
  reason        TEXT NOT NULL,
  used          INTEGER NOT NULL,
  at            INTEGER NOT NULL
);
