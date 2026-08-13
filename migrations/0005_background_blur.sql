-- A pre-blurred copy of the background, produced once at upload.
--
-- The card's frosted panels need a blurred backdrop. Doing that with SVG
-- filters costs ~160ms per render because each panel re-runs the blur; storing
-- the blurred image instead keeps rendering at ~13ms.
ALTER TABLE profile ADD COLUMN IF NOT EXISTS background_blur BYTEA;
