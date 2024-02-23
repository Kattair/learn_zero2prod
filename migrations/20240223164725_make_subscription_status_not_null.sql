-- Backfill subscriptions status and make it not null 
-- We wrap the migration in transaction because 'sqlx' does not do that for us
BEGIN;
    -- backfill
    UPDATE t_subscriptions
    SET status = 'confirmed'
    WHERE status is null;
    -- make not null
    ALTER TABLE t_subscriptions ALTER COLUMN status SET NOT NULL;
END;