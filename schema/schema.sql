CREATE TABLE paths (
    id INTEGER PRIMARY KEY,
    path TEXT UNIQUE NOT NULL,
    status INTEGER NOT NULL,
    location TEXT,
    content_type TEXT,
    filename TEXT NOT NULL
);