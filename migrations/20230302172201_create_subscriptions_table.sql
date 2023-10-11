-- Create Subscriptions Table
CREATE TABLE t_subscriptions (
    id uuid NOT NULL,
    email VARCHAR NOT NULL UNIQUE,
    name VARCHAR NOT NULL,
    subscribed_at TIMESTAMP NOT NULL,

    PRIMARY KEY(id)
)
