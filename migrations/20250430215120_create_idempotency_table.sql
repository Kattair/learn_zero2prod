CREATE TYPE header_pair AS (name TEXT, value BYTEA);
CREATE TABLE t_idempotency (
    user_id uuid NOT NULL REFERENCES t_users(user_id),
    idempotency_key VARCHAR NOT NULL,
    response_status_code SMALLINT NOT NULL,
    response_headers header_pair [] NOT NULL,
    response_body BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, idempotency_key)
);