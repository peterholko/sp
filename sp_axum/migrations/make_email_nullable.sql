-- Make email column nullable to support fingerprint-only account creation
ALTER TABLE accounts ALTER COLUMN email DROP NOT NULL;
