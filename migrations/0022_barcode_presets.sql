-- Barcode presets: barcodes pre-assigned as containers or items before scanning.
-- When a preset barcode is scanned in a stocker session, the system auto-creates
-- the record without requiring manual name entry (barcode becomes the name).
CREATE TABLE barcode_presets (
    barcode         TEXT        PRIMARY KEY,
    is_container    BOOLEAN     NOT NULL DEFAULT FALSE,
    container_type_id UUID      REFERENCES container_types(id) ON DELETE SET NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
