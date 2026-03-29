ALTER TABLE crawlers DROP COLUMN IF EXISTS join_key_hash;

DELETE FROM system_config WHERE key = 'join_key';
