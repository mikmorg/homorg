use sqlx::PgPool;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::{AppError, AppResult};
use crate::events::store::EventStore;
use crate::models::barcode::{BarcodeResolution, GeneratedBarcode};
use crate::models::event::{BarcodeGeneratedData, DomainEvent, EventMetadata};

/// Command handler for barcode generation and resolution.
#[derive(Clone)]
pub struct BarcodeCommands {
    pool: PgPool,
    config: AppConfig,
    event_store: EventStore,
}

impl BarcodeCommands {
    pub fn new(pool: PgPool, config: AppConfig, event_store: EventStore) -> Self {
        Self { pool, config, event_store }
    }

    /// Stable aggregate UUID for barcode-sequence events — derived from prefix name.
    fn sequence_aggregate_id(&self) -> Uuid {
        Uuid::new_v5(&Uuid::NAMESPACE_DNS, self.config.barcode_prefix.as_bytes())
    }

    /// Generate a single new system barcode.
    pub async fn generate_barcode(&self) -> AppResult<GeneratedBarcode> {
        // EH-1: Use fetch_optional so a missing prefix produces a clear error, not an opaque 500.
        let next: Option<i64> = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + 1 WHERE prefix = $1 RETURNING next_value - 1",
        )
        .bind(&self.config.barcode_prefix)
        .fetch_optional(&self.pool)
        .await?;

        let next = next.ok_or_else(|| AppError::Internal(format!(
            "Barcode prefix '{}' is not seeded in barcode_sequences. \
             Add a row or set BARCODE_PREFIX to a seeded value.",
            self.config.barcode_prefix
        )))?;

        let barcode = format!(
            "{}-{:0>width$}",
            self.config.barcode_prefix,
            next,
            width = self.config.barcode_pad_width
        );

        // DI-3: Emit BarcodeGenerated event for audit trail / event-sourced sequence log.
        let event = DomainEvent::BarcodeGenerated(BarcodeGeneratedData {
            barcode: barcode.clone(),
            assigned_to: None,
        });
        self.event_store
            .append(self.sequence_aggregate_id(), &event, Uuid::nil(), &EventMetadata::default())
            .await?;

        Ok(GeneratedBarcode { barcode })
    }

    /// Generate a single new system barcode within an existing transaction.
    pub async fn generate_barcode_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> AppResult<GeneratedBarcode> {
        // EH-1: fetch_optional for clear error on unconfigured prefix.
        let next: Option<i64> = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + 1 WHERE prefix = $1 RETURNING next_value - 1",
        )
        .bind(&self.config.barcode_prefix)
        .fetch_optional(&mut **tx)
        .await?;

        let next = next.ok_or_else(|| AppError::Internal(format!(
            "Barcode prefix '{}' is not seeded in barcode_sequences. \
             Add a row or set BARCODE_PREFIX to a seeded value.",
            self.config.barcode_prefix
        )))?;

        let barcode = format!(
            "{}-{:0>width$}",
            self.config.barcode_prefix,
            next,
            width = self.config.barcode_pad_width
        );

        // DI-3: Emit BarcodeGenerated event within the same transaction.
        let event = DomainEvent::BarcodeGenerated(BarcodeGeneratedData {
            barcode: barcode.clone(),
            assigned_to: None,
        });
        self.event_store
            .append_in_tx(tx, self.sequence_aggregate_id(), &event, Uuid::nil(), &EventMetadata::default())
            .await?;

        Ok(GeneratedBarcode { barcode })
    }

    /// Generate a batch of system barcodes.
    pub async fn generate_batch(&self, count: u32) -> AppResult<Vec<GeneratedBarcode>> {
        if count == 0 || count > 10000 {
            return Err(AppError::BadRequest(
                "Batch count must be between 1 and 10000".into(),
            ));
        }

        // EH-1: fetch_optional for clear error on unconfigured prefix.
        let start: Option<i64> = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + $1 WHERE prefix = $2 RETURNING next_value - $1",
        )
        .bind(count as i64)
        .bind(&self.config.barcode_prefix)
        .fetch_optional(&self.pool)
        .await?;

        let start = start.ok_or_else(|| AppError::Internal(format!(
            "Barcode prefix '{}' is not seeded in barcode_sequences.",
            self.config.barcode_prefix
        )))?;

        let barcodes: Vec<GeneratedBarcode> = (start..start + count as i64)
            .map(|n| GeneratedBarcode {
                barcode: format!(
                    "{}-{:0>width$}",
                    self.config.barcode_prefix,
                    n,
                    width = self.config.barcode_pad_width
                ),
            })
            .collect();

        // DI-3: Emit a single BarcodeGenerated event for the first barcode in the batch
        // (batch range is implicit from next_value).  One event per batch avoids write amplification.
        if let Some(first) = barcodes.first() {
            let event = DomainEvent::BarcodeGenerated(BarcodeGeneratedData {
                barcode: format!("{}..+{}", first.barcode, count),
                assigned_to: None,
            });
            self.event_store
                .append(self.sequence_aggregate_id(), &event, Uuid::nil(), &EventMetadata::default())
                .await?;
        }

        Ok(barcodes)
    }

    /// Resolve a scanned barcode string.
    pub async fn resolve_barcode(&self, code: &str) -> AppResult<BarcodeResolution> {
        let prefix_with_dash = format!("{}-", self.config.barcode_prefix);

        if code.starts_with(&prefix_with_dash) {
            // System barcode — look up in items
            let item_id: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM items WHERE system_barcode = $1 AND is_deleted = FALSE",
            )
            .bind(code)
            .fetch_optional(&self.pool)
            .await?;

            match item_id {
                Some(id) => Ok(BarcodeResolution::System {
                    barcode: code.to_string(),
                    item_id: id,
                }),
                None => Ok(BarcodeResolution::UnknownSystem {
                    barcode: code.to_string(),
                }),
            }
        } else {
            // Attempt to identify as a commercial code
            let code_type = classify_commercial_code(code);

            // Check if any item has this external code
            let found: Option<Uuid> = sqlx::query_scalar(
                "SELECT id FROM items WHERE external_codes @> $1::jsonb AND is_deleted = FALSE LIMIT 1",
            )
            .bind(serde_json::json!([{"value": code}]))
            .fetch_optional(&self.pool)
            .await?;

            if let Some(ct) = code_type {
                Ok(BarcodeResolution::External {
                    code_type: ct.to_string(),
                    value: code.to_string(),
                    item_id: found,
                })
            } else {
                Ok(BarcodeResolution::Unknown {
                    value: code.to_string(),
                })
            }
        }
    }
}

/// Heuristic classification of commercial barcodes.
pub(crate) fn classify_commercial_code(code: &str) -> Option<&'static str> {
    let digits_only = code.chars().all(|c| c.is_ascii_digit());
    if !digits_only {
        return None;
    }
    // CB-4: Keep arms unambiguous. ISBN-13 is exactly 13 digits with an 978/979 prefix.
    // A 14-digit code is always GTIN-14 regardless of prefix.  The old `10 | 14 if
    // code.starts_with("978")` arm was dead code for the 14-digit case and confusing
    // for the 10-digit case (ISBN-10 does not start with "978").
    match code.len() {
        12 => Some("UPC"),
        13 if code.starts_with("978") || code.starts_with("979") => Some("ISBN"),
        13 => Some("EAN"),
        10 => Some("ISBN"), // ISBN-10 legacy format
        14 => Some("GTIN"),
        8 => Some("EAN8"),
        _ => Some("BARCODE"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_upc_12_digits() {
        assert_eq!(classify_commercial_code("012345678901"), Some("UPC"));
    }

    #[test]
    fn classify_ean_13_digits() {
        assert_eq!(classify_commercial_code("0123456789012"), Some("EAN"));
    }

    #[test]
    fn classify_isbn_10_digits() {
        assert_eq!(classify_commercial_code("0123456789"), Some("ISBN"));
    }

    #[test]
    fn classify_isbn_13_prefix_978() {
        // 13-digit EAN starting with 978 → ISBN-13
        assert_eq!(classify_commercial_code("9781234567890"), Some("ISBN"));
    }

    #[test]
    fn classify_isbn_13_prefix_979() {
        assert_eq!(classify_commercial_code("9791234567890"), Some("ISBN"));
    }

    #[test]
    fn classify_gtin_14_with_978_prefix_is_not_isbn() {
        // CB-4: 14-digit codes are always GTIN regardless of prefix
        assert_eq!(classify_commercial_code("97812345678901"), Some("GTIN"));
    }

    #[test]
    fn classify_ean8() {
        assert_eq!(classify_commercial_code("01234567"), Some("EAN8"));
    }

    #[test]
    fn classify_gtin_14_digits() {
        assert_eq!(classify_commercial_code("01234567890123"), Some("GTIN"));
    }

    #[test]
    fn classify_non_digit_returns_none() {
        assert_eq!(classify_commercial_code("ABC-12345"), None);
    }

    #[test]
    fn classify_unknown_length_returns_barcode() {
        assert_eq!(classify_commercial_code("12345"), Some("BARCODE"));
    }
}
