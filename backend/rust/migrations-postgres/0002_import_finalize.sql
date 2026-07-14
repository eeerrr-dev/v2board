-- Finalize constraints and indexes after the one-time legacy COPY finishes.
-- Primary keys and row-local CHECK constraints remain in the preload schema;
-- this phase adds only cross-row uniqueness, secondary indexes, and foreign
-- keys that complete the single first-release PostgreSQL catalog.

ALTER TABLE payment_method
    ADD CONSTRAINT uniq_payment_method_driver_uuid UNIQUE (payment, uuid);
ALTER TABLE users
    ADD CONSTRAINT uniq_user_token UNIQUE (token);
ALTER TABLE orders
    ADD CONSTRAINT uniq_order_trade_no UNIQUE (trade_no);
ALTER TABLE stat
    ADD CONSTRAINT uniq_stat_record_at UNIQUE (record_at);

CREATE INDEX idx_plan_group_id ON plan(group_id);
CREATE INDEX idx_payment_method_archived ON payment_method(archived_at, id);

-- Legacy utf8mb4_unicode_ci treated human-entered redemption codes as
-- case-insensitive. Keep the submitted spelling for display while preventing
-- a second spelling that would have represented the same legacy code.
CREATE UNIQUE INDEX uniq_coupon_code_canonical ON coupon((lower(code)));

CREATE INDEX idx_user_plan_id ON users(plan_id);
-- Legacy MySQL's user-email uniqueness was case-insensitive. Preserve that
-- externally visible identity contract while retaining the submitted spelling
-- for display; the legacy converter must report broader collation collisions
-- before it reaches this index.
CREATE UNIQUE INDEX uniq_user_email_canonical ON users((lower(btrim(email))));
CREATE INDEX idx_user_group_id ON users(group_id);
CREATE INDEX idx_user_invite_user_id ON users(invite_user_id);
CREATE INDEX idx_user_renewal_candidate ON users(auto_renewal, expired_at, id);
CREATE INDEX idx_user_created_at ON users(created_at);

CREATE INDEX idx_order_user ON orders(user_id);
CREATE INDEX idx_order_user_status ON orders(user_id, status);
CREATE UNIQUE INDEX uniq_unfinished_order_per_user
    ON orders(user_id) WHERE status IN (0, 1);
CREATE INDEX idx_commission_claim ON orders(commission_status, id);
CREATE INDEX idx_order_payment_status ON orders(payment_id, status);
CREATE INDEX idx_order_status_id ON orders(status, id);
CREATE INDEX idx_order_referenced_plan ON orders(referenced_plan_id);
CREATE INDEX idx_order_coupon_user_status ON orders(coupon_id, user_id, status);
CREATE INDEX idx_order_created_at ON orders(created_at);
CREATE INDEX idx_order_paid_at ON orders(paid_at) WHERE paid_at IS NOT NULL;

CREATE INDEX idx_commission_log_inviter ON commission_log(invite_user_id, created_at DESC);
CREATE INDEX idx_commission_log_created_at ON commission_log(created_at);
CREATE UNIQUE INDEX uniq_invite_code_canonical ON invite_code((lower(code)));
CREATE INDEX idx_invite_user_status ON invite_code(user_id, status);
CREATE UNIQUE INDEX uniq_gift_card_code_canonical ON gift_card((lower(code)));
CREATE INDEX idx_gift_card_plan_id ON gift_card(plan_id);
CREATE INDEX idx_gift_card_redemption_user ON gift_card_redemption(user_id);
CREATE UNIQUE INDEX uniq_ticket_open_user ON ticket(user_id) WHERE status = 0;
CREATE INDEX idx_ticket_user_status ON ticket(user_id, status);
CREATE INDEX idx_ticket_auto_close ON ticket(status, reply_status, updated_at, id);
CREATE INDEX idx_ticket_message_ticket_id_id ON ticket_message(ticket_id, id);

ALTER TABLE plan
    ADD CONSTRAINT plan_group_id_fkey
    FOREIGN KEY (group_id) REFERENCES server_group(id) ON DELETE RESTRICT;
ALTER TABLE users
    ADD CONSTRAINT users_group_id_fkey
    FOREIGN KEY (group_id) REFERENCES server_group(id) ON DELETE RESTRICT,
    ADD CONSTRAINT users_plan_id_fkey
    FOREIGN KEY (plan_id) REFERENCES plan(id) ON DELETE RESTRICT,
    ADD CONSTRAINT fk_user_inviter
    FOREIGN KEY (invite_user_id) REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE orders
    ADD CONSTRAINT orders_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT,
    ADD CONSTRAINT orders_referenced_plan_id_fkey
    FOREIGN KEY (referenced_plan_id) REFERENCES plan(id) ON DELETE RESTRICT;
ALTER TABLE invite_code
    ADD CONSTRAINT invite_code_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE gift_card
    ADD CONSTRAINT gift_card_plan_id_fkey
    FOREIGN KEY (plan_id) REFERENCES plan(id) ON DELETE RESTRICT;
ALTER TABLE gift_card_redemption
    ADD CONSTRAINT gift_card_redemption_giftcard_id_fkey
    FOREIGN KEY (giftcard_id) REFERENCES gift_card(id) ON DELETE CASCADE,
    ADD CONSTRAINT gift_card_redemption_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE ticket
    ADD CONSTRAINT ticket_user_id_fkey
    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE RESTRICT;
ALTER TABLE ticket_message
    ADD CONSTRAINT ticket_message_ticket_id_fkey
    FOREIGN KEY (ticket_id) REFERENCES ticket(id) ON DELETE CASCADE;
