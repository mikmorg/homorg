use sqlx::PgPool;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::{AppError, AppResult};
use crate::models::barcode::{BarcodeResolution, GeneratedBarcode};

/// Command handler for barcode generation and resolution.
#[derive(Clone)]
pub struct BarcodeCommands {
    pool: PgPool,
    config: AppConfig,
}

impl BarcodeCommands {
    pub fn new(pool: PgPool, config: AppConfig) -> Self {
        Self { pool, config }
    }

    /// Generate a single new system barcode.
    pub async fn generate_barcode(&self) -> AppResult<GeneratedBarcode> {
        let next: i64 = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + 1 WHERE prefix = $1 RETURNING next_value - 1",
        )
        .bind(&self.config.barcode_prefix)
        .fetch_one(&self.pool)
        .await?;

        let barcode = format!(
            "{}-{:0>width$}",
            self.config.barcode_prefix,
            next,
            width = self.config.barcode_pad_width
        );

        Ok(GeneratedBarcode { barcode })
    }

    /// Generate a single new system barcode within an existing transaction.
    pub async fn generate_barcode_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    ) -> AppResult<GeneratedBarcode> {
        let next: i64 = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + 1 WHERE prefix = $1 RETURNING next_value - 1",
        )
        .bind(&self.config.barcode_prefix)
        .fetch_one(&mut **tx)
        .await?;

        let barcode = format!(
            "{}-{:0>width$}",
            self.config.barcode_prefix,
            next,
            width = self.config.barcode_pad_width
        );

        Ok(GeneratedBarcode { barcode })
    }

    /// Generate a batch of system barcodes.
    pub async fn generate_batch(&self, count: u32) -> AppResult<Vec<GeneratedBarcode>> {
        if count == 0 || count > 10000 {
            return Err(AppError::BadRequest(
                "Batch count must be between 1 and 10000".into(),
            ));
        }

        let start: i64 = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + $1 WHERE prefix = $2 RETURNING next_value - $1",
        )
        .bind(count as i64)
        .bind(&self.config.barcode_prefix)
        .fetch_one(&self.pool)
        .await?;

        let barcodes = (start..start + count as i64)
            .map(|n| GeneratedBarcode {
                barcode: format!(
                    "{}-{:0>width$}",
                    self.config.barcode_prefix,
                    n,
                    width = self.config.barcode_pad_width
                ),
            })
            .collect();

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
    match code.len() {
        12 => Some("UPC"),
        13 => Some("EAN"),
        10 | 14 if code.starts_with("978") || code.starts_with("979") => Some("ISBN"),
        10 => Some("ISBN"),
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
        assert_eq!(classify_commercial_code("97812345678901"), Some("ISBN"));
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
