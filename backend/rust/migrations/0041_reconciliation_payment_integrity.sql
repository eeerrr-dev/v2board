-- Prevent direct SQL deletion from orphaning verification/audit history.
ALTER TABLE `v2_payment_reconciliation`
    ADD CONSTRAINT `fk_payment_reconciliation_payment`
        FOREIGN KEY (`payment_id`) REFERENCES `v2_payment` (`id`) ON DELETE RESTRICT;
