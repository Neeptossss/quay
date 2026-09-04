CREATE UNIQUE INDEX idx_saved_view_name ON saved_view(name);

CREATE UNIQUE INDEX idx_saved_view_shortcut ON saved_view(shortcut)
  WHERE shortcut IS NOT NULL;
