CREATE TABLE device_tokens (
    id SERIAL PRIMARY KEY,
    player_id INTEGER NOT NULL REFERENCES accounts(player_id),
    token VARCHAR(64) NOT NULL UNIQUE,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_device_tokens_token ON device_tokens(token);
CREATE INDEX idx_device_tokens_player_id ON device_tokens(player_id);
