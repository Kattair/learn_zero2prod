CREATE TABLE t_users (
    user_id uuid PRIMARY KEY,
    username VARCHAR NOT NULL UNIQUE,
    password VARCHAR NOT NULL
);