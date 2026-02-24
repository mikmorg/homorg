-- 0004: Event store — append-only immutable ledger

CREATE TABLE event_store (
    id              BIGSERIAL PRIMARY KEY,
    event_id        UUID NOT NULL DEFAULT uuid_generate_v4(),
    aggregate_id    UUID NOT NULL,
    aggregate_type  VARCHAR(32) NOT NULL DEFAULT 'item',
    event_type      VARCHAR(64) NOT NULL,
    event_data      JSONB NOT NULL,
    metadata        JSONB NOT NULL DEFAULT '{}'::jsonb,
    actor_id        UUID REFERENCES users(id),
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    sequence_number BIGINT NOT NULL
);

-- Optimistic concurrency: one sequence per aggregate
ALTER TABLE event_store
    ADD CONSTRAINT uq_event_store_aggregate_seq
    UNIQUE (aggregate_id, sequence_number);

-- Replay a single item's full history
CREATE INDEX idx_event_store_aggregate_seq
    ON event_store (aggregate_id, sequence_number);

-- Filter by event kind
CREATE INDEX idx_event_store_event_type
    ON event_store (event_type);

-- Time-range audit queries
CREATE INDEX idx_event_store_created_at
    ON event_store (created_at);

-- Per-actor activity
CREATE INDEX idx_event_store_actor_id
    ON event_store (actor_id);

-- Correlate stocker session events
CREATE INDEX idx_event_store_session_id
    ON event_store ((metadata->>'session_id'))
    WHERE metadata->>'session_id' IS NOT NULL;

-- Prevent any UPDATE or DELETE on the event store
CREATE OR REPLACE FUNCTION prevent_event_store_mutation() RETURNS trigger AS $$
BEGIN
    RAISE EXCEPTION 'event_store is append-only: % operations are prohibited', TG_OP;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER trg_event_store_immutable_update
    BEFORE UPDATE ON event_store
    FOR EACH ROW
    EXECUTE FUNCTION prevent_event_store_mutation();

CREATE TRIGGER trg_event_store_immutable_delete
    BEFORE DELETE ON event_store
    FOR EACH ROW
    EXECUTE FUNCTION prevent_event_store_mutation();
