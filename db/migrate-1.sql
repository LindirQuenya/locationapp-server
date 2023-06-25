BEGIN;
ALTER TABLE api_keys RENAME TO api_keys_old;
ALTER TABLE api_keys_old ADD COLUMN username TEXT NOT NULL DEFAULT 'John';
CREATE TABLE api_keys(
  id INTEGER PRIMARY KEY,
  username TEXT NOT NULL,
  key_base64 TEXT NOT NULL,
  issued INTEGER NOT NULL,
  expiration INTEGER NOT NULL
);
INSERT INTO api_keys SELECT ALL FROM api_keys_old;
COMMIT;