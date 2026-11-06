-- Takes a database to one where each item that others depend on has one main dependency, the one it stands under in
-- the tree of items: the column main of item_links, set on the earliest made of each item's dependencies, and the
-- triggers that keep exactly one. Each statement below that defines something is written exactly as db/schema.sql
-- writes it.

ALTER TABLE item_links ADD COLUMN main INTEGER NOT NULL DEFAULT 0 CHECK (main IN (0, 1));

UPDATE item_links SET main = 1
WHERE rowid IN (SELECT min(rowid) FROM item_links GROUP BY depends_on);

-- A dependency made main, or the first made on an item, is the item's main one, and no other is.
CREATE TRIGGER item_links_main_made AFTER INSERT ON item_links
WHEN NEW.main = 1 OR NOT EXISTS (
  SELECT 1 FROM item_links WHERE depends_on = NEW.depends_on AND main = 1 AND item <> NEW.item) BEGIN
  UPDATE item_links SET main = (item = NEW.item) WHERE depends_on = NEW.depends_on;
END;

-- A dependency set main takes the place of the item's main one before it.
CREATE TRIGGER item_links_main_moved AFTER UPDATE OF main ON item_links
WHEN OLD.main = 0 AND NEW.main = 1 BEGIN
  UPDATE item_links SET main = 0 WHERE depends_on = NEW.depends_on AND item <> NEW.item AND main = 1;
END;

-- An item's main dependency is not unset while it stands: another is set main in its place.
CREATE TRIGGER item_links_main_kept AFTER UPDATE OF main ON item_links
WHEN OLD.main = 1 AND NEW.main = 0
  AND NOT EXISTS (SELECT 1 FROM item_links WHERE depends_on = OLD.depends_on AND main = 1) BEGIN
  SELECT RAISE(ABORT, 'an item others depend on keeps one main dependency: set another main in its place');
END;

-- When an item's main dependency goes, the earliest made of those left is its main one.
CREATE TRIGGER item_links_main_gone AFTER DELETE ON item_links WHEN OLD.main = 1 BEGIN
  UPDATE item_links SET main = 1
  WHERE rowid = (SELECT min(rowid) FROM item_links WHERE depends_on = OLD.depends_on);
END;

