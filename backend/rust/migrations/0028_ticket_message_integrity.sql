ALTER TABLE `v2_ticket_message`
    ADD CONSTRAINT `fk_ticket_message_ticket`
        FOREIGN KEY (`ticket_id`) REFERENCES `v2_ticket` (`id`) ON DELETE CASCADE;
