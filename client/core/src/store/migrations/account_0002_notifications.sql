-- Account database v2 (§12.5b, D27 = a): what this device has asked the OS to schedule.
-- `id` is the stable 31-bit notification ID derived from (task block ID, remind_at); `key`
-- is the readable form `<task id>@<remind_at>`.
CREATE TABLE scheduled_notifications (
    id INTEGER PRIMARY KEY,
    key TEXT NOT NULL UNIQUE,
    task_id TEXT NOT NULL,
    remind_at TEXT NOT NULL,
    fire_at TEXT NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('requested', 'scheduled', 'failed', 'shown')),
    last_result TEXT,
    updated_at TEXT NOT NULL
);
CREATE INDEX scheduled_notifications_task ON scheduled_notifications (task_id);
