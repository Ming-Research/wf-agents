-- Takes a version 31 database to version 32: the notice that an item a work's item depended on was cancelled names that
-- item, so that the cells where the work waits take it to choose the way on by a row of their own.

ALTER TABLE interjections ADD COLUMN cancelled_item INTEGER REFERENCES items(id);
