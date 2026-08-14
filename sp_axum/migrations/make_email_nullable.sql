-- Make email nullable so players can begin with a passwordless guest account.
ALTER TABLE accounts ALTER COLUMN email DROP NOT NULL;
