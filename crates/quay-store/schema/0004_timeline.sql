CREATE TABLE timeline_event (
  id            INTEGER PRIMARY KEY,
  pr_id         INTEGER NOT NULL REFERENCES pull_request(id) ON DELETE CASCADE,
  node_id       TEXT NOT NULL UNIQUE,
  kind          TEXT NOT NULL,
  actor         TEXT NOT NULL,
  body          TEXT,
  reference     TEXT,
  created_at    TEXT NOT NULL
);

CREATE INDEX idx_timeline_by_pull_request ON timeline_event(pr_id, created_at);
