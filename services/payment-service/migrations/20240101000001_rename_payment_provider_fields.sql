ALTER TABLE payments RENAME COLUMN stripe_payment_intent_id TO payment_provider_ref;

CREATE INDEX IF NOT EXISTS payments_provider_ref_idx ON payments (payment_provider_ref);