//! Integration tests for barcode commands.

mod common;

use homorg::constants::ROOT_ID;
use homorg::models::barcode::BarcodeResolution;
use homorg::models::event::EventMetadata;
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn generate_barcode_increments_sequence() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let bc1 = state.barcode_commands.generate_barcode().await.unwrap();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();

    assert!(bc1.barcode.starts_with("HOM-"));
    assert!(bc2.barcode.starts_with("HOM-"));
    assert_ne!(bc1.barcode, bc2.barcode);

    // Parse numeric portions and verify sequential
    let num1: i64 = bc1.barcode.strip_prefix("HOM-").unwrap().parse().unwrap();
    let num2: i64 = bc2.barcode.strip_prefix("HOM-").unwrap().parse().unwrap();
    assert_eq!(num2, num1 + 1);
}

#[tokio::test]
#[ignore]
async fn generate_batch_returns_correct_count() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let batch = state.barcode_commands.generate_batch(5).await.unwrap();
    assert_eq!(batch.len(), 5);

    // All unique
    let barcodes: Vec<_> = batch.iter().map(|b| &b.barcode).collect();
    let unique: std::collections::HashSet<_> = barcodes.iter().collect();
    assert_eq!(unique.len(), 5);
}

#[tokio::test]
#[ignore]
async fn generate_batch_rejects_zero() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let result = state.barcode_commands.generate_batch(0).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn resolve_system_barcode() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Resolvable", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let resolution = state.barcode_commands.resolve_barcode(&bc.barcode).await.unwrap();

    match resolution {
        BarcodeResolution::System { item_id: rid, .. } => assert_eq!(rid, item_id),
        other => panic!("Expected System resolution, got {other:?}"),
    }
}

#[tokio::test]
#[ignore]
async fn resolve_unknown_system_barcode() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    // Resolve a system-prefixed barcode that doesn't exist
    let resolution = state.barcode_commands.resolve_barcode("HOM-999999").await.unwrap();

    match resolution {
        BarcodeResolution::UnknownSystem { .. } => {}
        other => panic!("Expected UnknownSystem, got {other:?}"),
    }
}

#[tokio::test]
#[ignore]
async fn resolve_non_system_code_classifies() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    // A 13-digit code should be classified as EAN
    let resolution = state.barcode_commands.resolve_barcode("0123456789012").await.unwrap();

    match resolution {
        BarcodeResolution::External { code_type, .. } => assert_eq!(code_type, "EAN"),
        other => panic!("Expected External/EAN, got {other:?}"),
    }
}
