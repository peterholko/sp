-- Siege Perilous Database Initialization Script
-- Creates the sp schema with all tables, sequences, indexes, and constraints.
-- Derived from sp_axum and sp_server/network.rs source code.

BEGIN;

-- =============================================================================
-- accounts: Player account management
-- =============================================================================

CREATE TABLE IF NOT EXISTS accounts (
    player_id       INTEGER         NOT NULL,
    account_name    VARCHAR(50)     UNIQUE,
    password        VARCHAR(1000),
    email           VARCHAR(255)    UNIQUE,
    fingerprint     VARCHAR(64)     UNIQUE,
    created_at      TIMESTAMPTZ     NOT NULL,
    last_login      TIMESTAMPTZ,
    player_state    TEXT            NOT NULL DEFAULT 'CREATING_HERO',
    hero_name       VARCHAR(50),
    is_admin        BOOLEAN
);

CREATE SEQUENCE IF NOT EXISTS accounts_user_id_seq
    AS INTEGER
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1;

ALTER SEQUENCE accounts_user_id_seq OWNED BY accounts.player_id;

ALTER TABLE accounts
    ALTER COLUMN player_id SET DEFAULT nextval('accounts_user_id_seq');

ALTER TABLE accounts
    ADD CONSTRAINT accounts_pkey PRIMARY KEY (player_id);

CREATE INDEX IF NOT EXISTS idx_accounts_fingerprint ON accounts (fingerprint);

-- =============================================================================
-- sessions: Web and game client session tracking
-- =============================================================================

CREATE TABLE IF NOT EXISTS sessions (
    player_id   INTEGER         NOT NULL,
    session     VARCHAR(255)    NOT NULL,
    created_at  TIMESTAMPTZ     NOT NULL,
    last_login  TIMESTAMPTZ
);

-- =============================================================================
-- scores: Hero death / end-of-run leaderboard records
-- =============================================================================

CREATE TABLE IF NOT EXISTS scores (
    id          SERIAL          PRIMARY KEY,
    player_id   INTEGER         NOT NULL,
    hero_name   TEXT            NOT NULL,
    hero_rank   TEXT            NOT NULL,
    total_xp    INTEGER         NOT NULL,
    fate        TEXT            NOT NULL,
    created_at  TIMESTAMPTZ     DEFAULT NOW()
);

-- =============================================================================
-- device_tokens: Device-based authentication tokens
-- =============================================================================

CREATE TABLE IF NOT EXISTS device_tokens (
    id          SERIAL          PRIMARY KEY,
    player_id   INTEGER         NOT NULL REFERENCES accounts(player_id),
    token       VARCHAR(64)     NOT NULL UNIQUE,
    created_at  TIMESTAMP       NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_device_tokens_token ON device_tokens (token);
CREATE INDEX IF NOT EXISTS idx_device_tokens_player_id ON device_tokens (player_id);

COMMIT;
