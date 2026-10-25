-- Jetons d'accès en lecture seule pour une application tierce, Foodflow (ADR-0023).
CREATE TABLE access_token (
    id           TEXT PRIMARY KEY CHECK (length(id) = 36),
    user_id      TEXT NOT NULL REFERENCES app_user (id) ON DELETE CASCADE,
    name         TEXT NOT NULL UNIQUE CHECK (length(name) BETWEEN 1 AND 80),
    token_hash   TEXT NOT NULL UNIQUE,
    last_used_at TEXT CHECK (last_used_at IS NULL OR last_used_at LIKE '%Z'),
    created_at   TEXT NOT NULL CHECK (created_at LIKE '%Z'),
    updated_at   TEXT NOT NULL CHECK (updated_at LIKE '%Z')
) STRICT;
