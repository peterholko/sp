-- Add fingerprint column to accounts table for fingerprint-based authentication
-- This column stores a unique device fingerprint string (e.g., from FingerprintJS)
ALTER TABLE accounts ADD COLUMN fingerprint VARCHAR(64) UNIQUE;

-- Create an index on the fingerprint column for fast lookups
CREATE INDEX idx_accounts_fingerprint ON accounts (fingerprint);
