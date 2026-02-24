-- 0005: Barcode sequence generator

CREATE TABLE barcode_sequences (
    prefix      VARCHAR(8) PRIMARY KEY,
    next_value  BIGINT NOT NULL DEFAULT 1
);

-- Seed the default prefix
INSERT INTO barcode_sequences (prefix, next_value) VALUES ('HOM', 1);
