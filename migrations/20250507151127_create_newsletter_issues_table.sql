CREATE TABLE t_newsletter_issues (
    newsletter_issue_id uuid NOT NULL,
    title VARCHAR NOT NULL,
    text_content VARCHAR NOT NULL,
    html_content VARCHAR NOT NULL,
    published_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (newsletter_issue_id)
);