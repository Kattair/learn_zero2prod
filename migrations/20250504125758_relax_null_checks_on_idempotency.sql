ALTER TABLE t_idempotency
    ALTER COLUMN response_status_code DROP NOT NULL,
    ALTER COLUMN response_headers DROP NOT NULL,
    ALTER COLUMN response_body DROP NOT NULL;