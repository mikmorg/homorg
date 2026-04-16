use sqlx::PgPool;
use uuid::Uuid;

use crate::config::AppConfig;
use crate::errors::{AppError, AppResult};
use crate::events::projector::Projector;
use crate::events::store::EventStore;
use crate::models::barcode::{BarcodeResolution, GeneratedBarcode};
use crate::models::event::{BarcodeGeneratedData, DomainEvent, EventMetadata, ItemBarcodeAssignedData};

/// Command handler for barcode generation and resolution.
#[derive(Clone)]
pub struct BarcodeCommands {
    pool: PgPool,
    config: AppConfig,
    event_store: EventStore,
}

impl BarcodeCommands {
    pub fn new(pool: PgPool, config: AppConfig, event_store: EventStore) -> Self {
        Self {
            pool,
            config,
            event_store,
        }
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

        let next = next.ok_or_else(|| {
            AppError::Internal(format!(
                "Barcode prefix '{}' is not seeded in barcode_sequences. \
             Add a row or set BARCODE_PREFIX to a seeded value.",
                self.config.barcode_prefix
            ))
        })?;

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
            .append(
                self.sequence_aggregate_id(),
                &event,
                Uuid::nil(),
                &EventMetadata::default(),
            )
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

        let next = next.ok_or_else(|| {
            AppError::Internal(format!(
                "Barcode prefix '{}' is not seeded in barcode_sequences. \
             Add a row or set BARCODE_PREFIX to a seeded value.",
                self.config.barcode_prefix
            ))
        })?;

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
            .append_in_tx(
                tx,
                self.sequence_aggregate_id(),
                &event,
                Uuid::nil(),
                &EventMetadata::default(),
            )
            .await?;

        Ok(GeneratedBarcode { barcode })
    }

    /// Generate a batch of system barcodes within an existing transaction.
    ///
    /// Identical to [`generate_batch`] but uses the provided transaction as executor so
    /// the sequence update and event store write are part of the caller's transaction.
    pub async fn generate_batch_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        count: u32,
    ) -> AppResult<Vec<GeneratedBarcode>> {
        if count == 0 || count > 10000 {
            return Err(AppError::BadRequest("Batch count must be between 1 and 10000".into()));
        }

        let start: Option<i64> = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + $1 WHERE prefix = $2 RETURNING next_value - $1",
        )
        .bind(count as i64)
        .bind(&self.config.barcode_prefix)
        .fetch_optional(&mut **tx)
        .await?;

        let start = start.ok_or_else(|| {
            AppError::Internal(format!(
                "Barcode prefix '{}' is not seeded in barcode_sequences.",
                self.config.barcode_prefix
            ))
        })?;

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

        if let Some(first) = barcodes.first() {
            let event = DomainEvent::BarcodeGenerated(BarcodeGeneratedData {
                barcode: format!("{}..+{}", first.barcode, count),
                assigned_to: None,
            });
            self.event_store
                .append_in_tx(
                    tx,
                    self.sequence_aggregate_id(),
                    &event,
                    Uuid::nil(),
                    &EventMetadata::default(),
                )
                .await?;
        }

        Ok(barcodes)
    }

    /// Generate a batch of system barcodes.
    pub async fn generate_batch(&self, count: u32) -> AppResult<Vec<GeneratedBarcode>> {
        if count == 0 || count > 10000 {
            return Err(AppError::BadRequest("Batch count must be between 1 and 10000".into()));
        }

        // EH-1: fetch_optional for clear error on unconfigured prefix.
        let start: Option<i64> = sqlx::query_scalar(
            "UPDATE barcode_sequences SET next_value = next_value + $1 WHERE prefix = $2 RETURNING next_value - $1",
        )
        .bind(count as i64)
        .bind(&self.config.barcode_prefix)
        .fetch_optional(&self.pool)
        .await?;

        let start = start.ok_or_else(|| {
            AppError::Internal(format!(
                "Barcode prefix '{}' is not seeded in barcode_sequences.",
                self.config.barcode_prefix
            ))
        })?;

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
                .append(
                    self.sequence_aggregate_id(),
                    &event,
                    Uuid::nil(),
                    &EventMetadata::default(),
                )
                .await?;
        }

        Ok(barcodes)
    }

    /// Resolve a scanned barcode string.
    pub async fn resolve_barcode(&self, code: &str) -> AppResult<BarcodeResolution> {
        let prefix_with_dash = format!("{}-", self.config.barcode_prefix);

        if code.starts_with(&prefix_with_dash) {
            // System barcode — look up in items
            let item_id: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM items WHERE system_barcode = $1 AND is_deleted = FALSE")
                    .bind(code)
                    .fetch_optional(&self.pool)
                    .await?;

            match item_id {
                Some(id) => Ok(BarcodeResolution::System {
                    barcode: code.to_string(),
                    item_id: id,
                }),
                None => {
                    // Check barcode_presets before returning UnknownSystem.
                    let preset: Option<(bool, Option<Uuid>, Option<String>)> = sqlx::query_as(
                        "SELECT bp.is_container, bp.container_type_id, ct.name \
                         FROM barcode_presets bp \
                         LEFT JOIN container_types ct ON ct.id = bp.container_type_id \
                         WHERE bp.barcode = $1",
                    )
                    .bind(code)
                    .fetch_optional(&self.pool)
                    .await?;

                    match preset {
                        Some((is_container, container_type_id, container_type_name)) => Ok(BarcodeResolution::Preset {
                            barcode: code.to_string(),
                            is_container,
                            container_type_id,
                            container_type_name,
                        }),
                        None => Ok(BarcodeResolution::UnknownSystem {
                            barcode: code.to_string(),
                        }),
                    }
                }
            }
        } else {
            // Attempt to identify as a commercial code
            let code_type = classify_commercial_code(code);

            // B1: When the code type is known, include it in the containment query so that
            // a UPC "012345678905" and an EAN "012345678905" (same digits, different types)
            // do not collide into a false multi-match. Fall back to value-only when the type
            // cannot be determined (e.g. free-form alphanumeric ASIN).
            let query_json = if let Some(ct) = code_type {
                serde_json::json!([{"type": ct, "value": code}])
            } else {
                serde_json::json!([{"value": code}])
            };

            // Check if any item(s) have this external code — collect all matches (multi-match).
            let found: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM items WHERE external_codes @> $1::jsonb AND is_deleted = FALSE ORDER BY created_at",
            )
            .bind(query_json)
            .fetch_all(&self.pool)
            .await?;

            if let Some(ct) = code_type {
                Ok(BarcodeResolution::External {
                    code_type: ct.to_string(),
                    value: code.to_string(),
                    item_ids: found,
                })
            } else if !found.is_empty() {
                // BC-R1: Non-classifiable code (e.g. alphanumeric ASIN) but items
                // matched by external_codes value — return them as untyped external.
                Ok(BarcodeResolution::External {
                    code_type: "BARCODE".to_string(),
                    value: code.to_string(),
                    item_ids: found,
                })
            } else {
                Ok(BarcodeResolution::Unknown {
                    value: code.to_string(),
                })
            }
        }
    }

    /// Resolve a scanned barcode string within an open transaction.
    ///
    /// Identical to [`resolve_barcode`] but uses the provided transaction as
    /// executor so that items created earlier in the same transaction are
    /// visible (important for atomic stocker batches where a `Resolve` event
    /// follows a `CreateAndPlace` in the same batch).
    pub async fn resolve_barcode_in_tx(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        code: &str,
    ) -> AppResult<BarcodeResolution> {
        let prefix_with_dash = format!("{}-", self.config.barcode_prefix);

        if code.starts_with(&prefix_with_dash) {
            let item_id: Option<Uuid> =
                sqlx::query_scalar("SELECT id FROM items WHERE system_barcode = $1 AND is_deleted = FALSE")
                    .bind(code)
                    .fetch_optional(&mut **tx)
                    .await?;

            match item_id {
                Some(id) => Ok(BarcodeResolution::System {
                    barcode: code.to_string(),
                    item_id: id,
                }),
                None => {
                    let preset: Option<(bool, Option<Uuid>, Option<String>)> = sqlx::query_as(
                        "SELECT bp.is_container, bp.container_type_id, ct.name \
                         FROM barcode_presets bp \
                         LEFT JOIN container_types ct ON ct.id = bp.container_type_id \
                         WHERE bp.barcode = $1",
                    )
                    .bind(code)
                    .fetch_optional(&mut **tx)
                    .await?;

                    match preset {
                        Some((is_container, container_type_id, container_type_name)) => Ok(BarcodeResolution::Preset {
                            barcode: code.to_string(),
                            is_container,
                            container_type_id,
                            container_type_name,
                        }),
                        None => Ok(BarcodeResolution::UnknownSystem {
                            barcode: code.to_string(),
                        }),
                    }
                }
            }
        } else {
            let code_type = classify_commercial_code(code);

            // B1: same type-aware query as resolve_barcode (see above).
            let query_json = if let Some(ct) = code_type {
                serde_json::json!([{"type": ct, "value": code}])
            } else {
                serde_json::json!([{"value": code}])
            };

            let found: Vec<Uuid> = sqlx::query_scalar(
                "SELECT id FROM items WHERE external_codes @> $1::jsonb AND is_deleted = FALSE ORDER BY created_at",
            )
            .bind(query_json)
            .fetch_all(&mut **tx)
            .await?;

            if let Some(ct) = code_type {
                Ok(BarcodeResolution::External {
                    code_type: ct.to_string(),
                    value: code.to_string(),
                    item_ids: found,
                })
            } else if !found.is_empty() {
                Ok(BarcodeResolution::External {
                    code_type: "BARCODE".to_string(),
                    value: code.to_string(),
                    item_ids: found,
                })
            } else {
                Ok(BarcodeResolution::Unknown {
                    value: code.to_string(),
                })
            }
        }
    }

    /// Assign a barcode to a specific item.
    ///
    /// Emits an `ItemBarcodeAssigned` event which records both the new barcode and the
    /// previous one so the undo system can reverse the assignment.
    pub async fn assign_barcode(
        &self,
        item_id: Uuid,
        barcode: &str,
        actor_id: Uuid,
        metadata: &EventMetadata,
    ) -> AppResult<crate::models::event::StoredEvent> {
        let mut tx = self.pool.begin().await?;

        // BC-1: Use FOR UPDATE to serialize concurrent barcode assignments to the same item so
        // that previous_barcode in the event always reflects the true prior state, preventing
        // undo chain corruption when two callers race.
        let current: Option<(Uuid, Option<String>)> =
            sqlx::query_as("SELECT id, system_barcode FROM items WHERE id = $1 AND is_deleted = FALSE FOR UPDATE")
                .bind(item_id)
                .fetch_optional(&mut *tx)
                .await?;

        let (_, previous_barcode) = current.ok_or_else(|| AppError::NotFound(format!("Item {item_id} not found")))?;

        // If assigning the same barcode, reject early.
        if previous_barcode.as_deref() == Some(barcode) {
            return Err(AppError::BadRequest("Item already has this barcode".into()));
        }

        // Ensure the new barcode is not already in use by another item.
        let taken_by: Option<Uuid> =
            sqlx::query_scalar("SELECT id FROM items WHERE system_barcode = $1 AND is_deleted = FALSE")
                .bind(barcode)
                .fetch_optional(&mut *tx)
                .await?;

        if let Some(owner) = taken_by {
            return Err(AppError::Conflict(format!(
                "Barcode '{barcode}' is already assigned to item {owner}"
            )));
        }

        let event = DomainEvent::ItemBarcodeAssigned(ItemBarcodeAssignedData {
            barcode: barcode.to_string(),
            previous_barcode: previous_barcode.clone(),
        });

        let stored = self
            .event_store
            .append_in_tx(&mut tx, item_id, &event, actor_id, metadata)
            .await?;
        Projector::apply(&mut tx, item_id, &event, actor_id).await?;
        self.event_store.commit_and_notify(tx).await?;

        Ok(stored)
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
        8 => Some("EAN-8"),
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
        assert_eq!(classify_commercial_code("01234567"), Some("EAN-8"));
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
