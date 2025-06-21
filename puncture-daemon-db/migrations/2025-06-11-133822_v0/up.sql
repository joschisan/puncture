CREATE TABLE user (
    user_pk TEXT NOT NULL PRIMARY KEY,
    invite_id TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE invite (
    id TEXT NOT NULL PRIMARY KEY,
    user_limit BIGINT NOT NULL,
    expires_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE invoice (
    id TEXT NOT NULL PRIMARY KEY,
    user_pk TEXT NOT NULL,
    amount_msat BIGINT,
    description TEXT NOT NULL,
    pr TEXT NOT NULL,
    expires_at BIGINT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE receive (
    id TEXT NOT NULL PRIMARY KEY,
    user_pk TEXT NOT NULL,
    amount_msat BIGINT NOT NULL,
    description TEXT NOT NULL,
    pr TEXT NOT NULL,
    created_at BIGINT NOT NULL
);

CREATE TABLE send (
    id TEXT NOT NULL PRIMARY KEY,
    user_pk TEXT NOT NULL,
    amount_msat BIGINT NOT NULL,
    fee_msat BIGINT NOT NULL,
    description TEXT NOT NULL,
    pr TEXT NOT NULL,
    status TEXT NOT NULL,
    ln_address TEXT,
    created_at BIGINT NOT NULL
);

CREATE TABLE offer (
    id TEXT NOT NULL PRIMARY KEY,
    user_pk TEXT NOT NULL,
    amount_msat BIGINT,
    description TEXT NOT NULL,
    pr TEXT NOT NULL,
    expires_at BIGINT,
    created_at BIGINT NOT NULL
);

CREATE INDEX idx_user_invite_id ON user(invite_id);
CREATE INDEX idx_invoice_user_pk ON invoice(user_pk);
CREATE INDEX idx_receive_user_pk ON receive(user_pk);
CREATE INDEX idx_send_user_pk ON send(user_pk);
CREATE INDEX idx_offer_user_pk ON offer(user_pk);
