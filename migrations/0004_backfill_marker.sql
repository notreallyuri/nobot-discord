-- Marks a profile as having been granted the coins and achievements it earned
-- before those systems existed. Null means the backfill has not run for them.
ALTER TABLE profile ADD COLUMN IF NOT EXISTS backfilled_at TIMESTAMPTZ;
