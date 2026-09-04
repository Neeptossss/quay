CREATE TABLE organization (
  id            INTEGER PRIMARY KEY,
  account_id    INTEGER NOT NULL REFERENCES account(id),
  login         TEXT NOT NULL,
  display_name  TEXT,
  is_member     INTEGER NOT NULL DEFAULT 1,
  UNIQUE(account_id, login)
);

CREATE INDEX idx_organization_by_account ON organization(account_id, login);

ALTER TABLE account ADD COLUMN selected_organization TEXT;
