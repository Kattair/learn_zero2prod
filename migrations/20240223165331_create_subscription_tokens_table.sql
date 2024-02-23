-- Create Subscription Tokens Table
CREATE TABLE t_subscription_tokens (
    subscription_token VARCHAR NOT NULL,
    subscriber_id uuid NOT NULL
        REFERENCES t_subscriptions(id),
    PRIMARY KEY (subscription_token)
);
