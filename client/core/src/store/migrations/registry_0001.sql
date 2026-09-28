-- Device registry (no account content): which accounts have a local database on this device,
-- which one is active, and device-wide defaults (device name).
CREATE TABLE accounts (
    user_id TEXT PRIMARY KEY,
    username TEXT NOT NULL,
    display_name TEXT NOT NULL,
    last_active_at TEXT NOT NULL,
    active INTEGER NOT NULL DEFAULT 0
);
CREATE TABLE device (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
