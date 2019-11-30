CREATE TABLE signatories (
  id INTEGER PRIMARY KEY,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  organisation TEXT,
  url TEXT,
  comment TEXT,
  mailing_list_opt_in INTEGER,
  created_on INTEGER NOT NULL
);