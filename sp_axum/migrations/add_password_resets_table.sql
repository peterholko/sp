CREATE TABLE password_resets (
    token       VARCHAR(64)     NOT NULL PRIMARY KEY,
    player_id   INTEGER         NOT NULL REFERENCES accounts(player_id),
    created_at  TIMESTAMPTZ     NOT NULL DEFAULT NOW(),
    used_at     TIMESTAMPTZ
);

CREATE INDEX idx_password_resets_player_id ON password_resets(player_id);
