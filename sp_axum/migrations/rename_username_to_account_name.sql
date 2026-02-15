-- Rename username column to account_name and make it nullable
ALTER TABLE accounts RENAME COLUMN username TO account_name;
ALTER TABLE accounts ALTER COLUMN account_name DROP NOT NULL;

-- Make password column nullable
ALTER TABLE accounts ALTER COLUMN password DROP NOT NULL;
