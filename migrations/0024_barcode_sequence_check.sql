-- N6: Add CHECK constraint to prevent barcode sequence counter from going negative.
-- The application only ever increments this counter, but a direct SQL UPDATE
-- could corrupt it without this guard.
ALTER TABLE barcode_sequences
    ADD CONSTRAINT chk_barcode_sequences_next_value_positive
    CHECK (next_value >= 1);
