-- This file should undo anything in `up.sql`

DROP INDEX idx_bolt11_send_user_pk;
DROP INDEX idx_bolt11_receive_user_pk;
DROP INDEX idx_bolt11_invoice_user_pk;

DROP TABLE bolt11_send;
DROP TABLE bolt11_receive;
DROP TABLE bolt11_invoice;
DROP TABLE users;
