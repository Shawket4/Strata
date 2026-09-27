-- Account columns that replace two interim encodings (PLAN §8, §12.5b, D27):
--   * devices.reminders_enabled: whether a device schedules local reminder notifications. It
--     was kept in the user's `settings` table under `device.<id>.reminders_enabled` (a
--     MessagePack boolean); those rows move into the column and are deleted.
--   * users.must_change_password: set by an admin password reset. It was encoded as a
--     `temporary:` prefix on `users.password_hash`; prefixed hashes are stripped and flagged.

ALTER TABLE devices ADD COLUMN reminders_enabled boolean NOT NULL DEFAULT true;
ALTER TABLE users ADD COLUMN must_change_password boolean NOT NULL DEFAULT false;

UPDATE users
SET must_change_password = true,
    password_hash = substr(password_hash, length('temporary:') + 1)
WHERE starts_with(password_hash, 'temporary:');

-- `settings` and `devices` are user-owned with forced RLS, which binds this migration's role
-- too: an unscoped statement sees no rows. Each user's rows are moved inside that user's scope
-- (transaction-local, like `AppDb::begin`); the scope is cleared at the end. Keys carry the
-- device ID as a ULID string (Crockford base32, 26 characters = 2 zero bits + 128 bits), which
-- is decoded to the uuid stored in `devices.id`.
DO $$
DECLARE
    alphabet CONSTANT text := '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
    u uuid;
    r record;
    bits text;
    i int;
    device uuid;
BEGIN
    FOR u IN SELECT id FROM users LOOP
        PERFORM set_config('strata.user_id', u::text, true);
        FOR r IN
            SELECT key, value FROM settings
            WHERE key ~ '^device\.[0-7][0-9A-HJKMNP-TV-Z]{25}\.reminders_enabled$'
        LOOP
            bits := '';
            FOR i IN 1..26 LOOP
                bits := bits || ((strpos(alphabet, substr(r.key, 7 + i, 1)) - 1)::bit(5))::text;
            END LOOP;
            device := (lpad(to_hex(CAST(substr(bits, 3, 64) AS bit(64))::bigint), 16, '0')
                    || lpad(to_hex(CAST(substr(bits, 67, 64) AS bit(64))::bigint), 16, '0'))::uuid;
            -- MessagePack `false` is 0xc2. Anything else (true, or a value the API could not
            -- decode) meant the default, `true`, and keeps the column default.
            IF r.value = '\xc2'::bytea THEN
                UPDATE devices SET reminders_enabled = false WHERE id = device;
            END IF;
        END LOOP;
        DELETE FROM settings WHERE key LIKE 'device.%.reminders_enabled';
    END LOOP;
    PERFORM set_config('strata.user_id', '', true);
END
$$;
