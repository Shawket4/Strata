-- Tasks parsed from Obsidian Tasks checklist lines (PLAN §6.11) and reminder delivery (D27).

CREATE TABLE tasks (
    user_id               uuid NOT NULL,
    -- The line's block ID without the caret, e.g. `t-01j9a2…`.
    id                    text NOT NULL,
    note_id               uuid NOT NULL,
    text                  text NOT NULL,
    status                text NOT NULL CHECK (status IN ('open', 'done', 'cancelled')),
    due                   date,
    scheduled             date,
    start                 date,
    recurrence_raw        text,
    rrule                 text,
    -- False when recurrence_raw is outside the supported grammar (kept verbatim, flagged).
    recurrence_understood boolean NOT NULL DEFAULT true,
    priority              text CHECK (priority IN ('highest', 'high', 'medium', 'low', 'lowest')),
    done_at               date,
    line_start            integer NOT NULL CHECK (line_start >= 0),
    line_end              integer NOT NULL,
    PRIMARY KEY (user_id, id),
    FOREIGN KEY (user_id, note_id) REFERENCES notes (user_id, id) ON DELETE CASCADE,
    CHECK (line_end >= line_start),
    CHECK (rrule IS NULL OR recurrence_raw IS NOT NULL)
);
CREATE INDEX tasks_note ON tasks (user_id, note_id, line_start);
CREATE INDEX tasks_due ON tasks (user_id, status, due);
SELECT strata_make_user_owned('tasks');

CREATE TABLE task_reminders (
    user_id   uuid NOT NULL,
    task_id   text NOT NULL,
    remind_at timestamptz NOT NULL,
    PRIMARY KEY (user_id, task_id, remind_at),
    FOREIGN KEY (user_id, task_id) REFERENCES tasks (user_id, id) ON DELETE CASCADE
);
CREATE INDEX task_reminders_due ON task_reminders (user_id, remind_at);
SELECT strata_make_user_owned('task_reminders');

-- One send per (task, remind_at, device). No foreign keys: delivery history outlives
-- re-derived tasks and removed devices.
CREATE TABLE notification_log (
    user_id   uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    id        uuid NOT NULL,
    task_id   text NOT NULL,
    remind_at timestamptz NOT NULL,
    device_id uuid NOT NULL,
    provider  text NOT NULL CHECK (provider IN ('fcm', 'apns', 'wns', 'event_stream')),
    sent_at   timestamptz NOT NULL,
    result    text NOT NULL CHECK (result IN ('sent', 'failed', 'invalid_token')),
    PRIMARY KEY (user_id, id),
    UNIQUE (user_id, task_id, remind_at, device_id)
);
SELECT strata_make_user_owned('notification_log');
