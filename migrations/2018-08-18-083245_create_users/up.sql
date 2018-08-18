CREATE TABLE users (
  id SERIAL PRIMARY KEY,
  username VARCHAR UNIQUE NOT NULL,
  realname VARCHAR NOT NULL,
  password VARCHAR UNIQUE NOT NULL
);

CREATE UNIQUE INDEX users_username_idx ON users (username);
