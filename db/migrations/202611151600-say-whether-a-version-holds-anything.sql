-- From version 17 of the schema to version 18: a write of a block says whether the version holds nothing, with no
-- value and its content empty or only whitespace, which a test of content reads. A write kept before is read from its
-- value and its content as kept. The version's one row is replaced last.
ALTER TABLE writes ADD COLUMN empty INTEGER NOT NULL DEFAULT 0 CHECK (empty IN (0, 1));
ALTER TABLE written ADD COLUMN empty INTEGER NOT NULL DEFAULT 0 CHECK (empty IN (0, 1));
UPDATE writes SET empty = 1 WHERE EXISTS (
    SELECT 1 FROM finished f JOIN contents c ON c.work = f.work AND c.version = f.at
    WHERE f.work = writes.work AND f.run = writes.run AND c.block = writes.block AND writes.value IS NULL
        AND trim(c.content, ' ' || char(9, 10, 11, 12, 13)) = ''
);
UPDATE written SET empty = 1 WHERE EXISTS (
    SELECT 1 FROM contents c
    WHERE c.work = written.work AND c.version = written.at AND c.block = written.block AND written.value IS NULL
        AND trim(c.content, ' ' || char(9, 10, 11, 12, 13)) = ''
);
UPDATE schema_version SET version = 18;
