CREATE TABLE t_issue_delivery_queue (
    newsletter_issue_id uuid NOT NULL
        REFERENCES t_newsletter_issues (newsletter_issue_id),
    subscriber_email VARCHAR NOT NULL,
    PRIMARY KEY (newsletter_issue_id, subscriber_email)
);