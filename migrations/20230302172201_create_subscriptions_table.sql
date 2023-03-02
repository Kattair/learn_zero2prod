-- Create Subscriptions Table
CREATE TABLE t_subscriptions (
    id uuid NOT NULL,
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    subscribed_at TIMESTAMP NOT NULL,

    PRIMARY KEY(id)
)
