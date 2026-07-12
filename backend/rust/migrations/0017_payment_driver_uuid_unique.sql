ALTER TABLE `v2_payment`
    ADD UNIQUE KEY `uniq_payment_driver_uuid` (`payment`, `uuid`);
