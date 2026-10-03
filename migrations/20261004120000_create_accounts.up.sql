CREATE TABLE account (
    id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    username     text        NOT NULL CHECK (username = lower(username) AND length(username) BETWEEN 1 AND 64),
    display_name text        NOT NULL DEFAULT '',
    role         text        NOT NULL CHECK (role IN ('viewer', 'editor', 'administrator')),
    disabled_at  timestamptz,
    created_at   timestamptz NOT NULL DEFAULT now(),
    updated_at   timestamptz NOT NULL DEFAULT now(),
    CONSTRAINT account_username_key UNIQUE (username)
);

CREATE TABLE identity (
    id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   uuid        NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    kind         text        NOT NULL CHECK (kind IN ('password')),
    subject      text,
    secret_hash  text,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_used_at timestamptz,
    CONSTRAINT identity_password_has_secret
        CHECK (kind <> 'password' OR (secret_hash IS NOT NULL AND subject IS NULL))
);

CREATE UNIQUE INDEX identity_one_password_per_account
    ON identity (account_id) WHERE kind = 'password';

CREATE UNIQUE INDEX identity_subject_key
    ON identity (kind, subject) WHERE subject IS NOT NULL;

CREATE TABLE session (
    token_hash   bytea       PRIMARY KEY,
    account_id   uuid        NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    created_at   timestamptz NOT NULL DEFAULT now(),
    last_seen_at timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz NOT NULL
);

CREATE INDEX session_account_id_idx ON session (account_id);

CREATE TABLE api_token (
    id           uuid        PRIMARY KEY DEFAULT gen_random_uuid(),
    account_id   uuid        NOT NULL REFERENCES account (id) ON DELETE CASCADE,
    name         text        NOT NULL,
    token_hash   bytea       NOT NULL UNIQUE,
    hint         text        NOT NULL,
    created_at   timestamptz NOT NULL DEFAULT now(),
    expires_at   timestamptz,
    last_used_at timestamptz,
    revoked_at   timestamptz
);

CREATE INDEX api_token_account_id_idx ON api_token (account_id);

CREATE TABLE audit_log (
    id             bigint      GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    occurred_at    timestamptz NOT NULL DEFAULT now(),
    actor_id       uuid        REFERENCES account (id) ON DELETE SET NULL,
    actor_username text        NOT NULL,
    action         text        NOT NULL,
    target_type    text        NOT NULL,
    target_id      uuid,
    details        jsonb       NOT NULL DEFAULT '{}'
);

CREATE INDEX audit_log_occurred_at_idx ON audit_log (occurred_at);
CREATE INDEX audit_log_target_idx ON audit_log (target_type, target_id);

CREATE FUNCTION audit_log_is_append_only() RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    RAISE EXCEPTION 'the audit log is append-only'
        USING ERRCODE = 'insufficient_privilege';
END
$$;

CREATE TRIGGER audit_log_append_only
    BEFORE UPDATE OR DELETE ON audit_log
    FOR EACH ROW
    EXECUTE FUNCTION audit_log_is_append_only();

CREATE TRIGGER audit_log_no_truncate
    BEFORE TRUNCATE ON audit_log
    FOR EACH STATEMENT
    EXECUTE FUNCTION audit_log_is_append_only();
