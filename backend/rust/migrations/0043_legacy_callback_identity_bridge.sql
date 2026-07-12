-- During a rolling deploy, an older API binary can update callback_no without
-- knowing callback_no_hash. Detect that exact old-writer shape and derive the
-- digest from its complete (legacy <=255-byte) value. New binaries update both
-- columns, so their digest of a longer raw identifier is preserved rather than
-- replaced with a digest of the bounded display label.
CREATE TRIGGER `v2_order_callback_identity_before_update`
BEFORE UPDATE ON `v2_order`
FOR EACH ROW
SET NEW.`callback_no_hash` = CASE
    WHEN NOT (NEW.`callback_no` <=> OLD.`callback_no`)
         AND (NEW.`callback_no_hash` <=> OLD.`callback_no_hash`)
    THEN UNHEX(SHA2(NEW.`callback_no`, 256))
    ELSE NEW.`callback_no_hash`
END;
