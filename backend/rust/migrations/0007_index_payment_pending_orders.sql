-- Keep payment lifecycle guards selective. Admin payment mutation and checkout
-- both lock the payment row first, then locate bound pending orders by this
-- prefix; without it MySQL can scan and next-key lock the entire order table.
ALTER TABLE `v2_order`
    ADD KEY `idx_order_payment_status` (`payment_id`, `status`);
