//! Integration tests for undo commands (compensating events).

mod common;

use homorg::constants::ROOT_ID;
use homorg::models::event::EventMetadata;
use homorg::models::item::{MoveItemRequest, UpdateItemRequest};
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn undo_create_soft_deletes() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "Undo Me", false);
    let created = state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Undo the creation → should soft-delete
    let compensating = state
        .undo_commands
        .undo_event(created.event_id, ctx.admin_id)
        .await
        .unwrap();
    assert_eq!(compensating.event_type, "ItemDeleted");

    let row: (bool,) =
        sqlx::query_as("SELECT is_deleted FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert!(row.0);
}

#[tokio::test]
#[ignore]
async fn undo_delete_restores() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Restore Me", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let deleted = state
        .item_commands
        .delete_item(item_id, Some("bye".into()), ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Undo the deletion → should restore
    let compensating = state
        .undo_commands
        .undo_event(deleted.event_id, ctx.admin_id)
        .await
        .unwrap();
    assert_eq!(compensating.event_type, "ItemRestored");

    let row: (bool,) =
        sqlx::query_as("SELECT is_deleted FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert!(!row.0);
}

#[tokio::test]
#[ignore]
async fn undo_move_reverts_path() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create two containers
    let box_a = Uuid::new_v4();
    let bc_a = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            box_a,
            &common::make_item_request(&bc_a.barcode, ROOT_ID, "Box A", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let box_b = Uuid::new_v4();
    let bc_b = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            box_b,
            &common::make_item_request(&bc_b.barcode, ROOT_ID, "Box B", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Create item in Box A
    let item_id = Uuid::new_v4();
    let bc_item = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc_item.barcode, box_a, "Moveable", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Move to Box B
    let moved = state
        .item_commands
        .move_item(
            item_id,
            &MoveItemRequest {
                container_id: box_b,
                coordinate: None,
            },
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Undo the move
    let compensating = state
        .undo_commands
        .undo_event(moved.event_id, ctx.admin_id)
        .await
        .unwrap();
    assert_eq!(compensating.event_type, "ItemMoveReverted");

    // Item should be back in Box A
    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    assert_eq!(item.item.parent_id, Some(box_a));
}

#[tokio::test]
#[ignore]
async fn undo_update_reverses_fields() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Original Name", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let update_req = UpdateItemRequest {
        name: Some("New Name".into()),
        description: None,
        category: None,
        tags: None,
        is_container: None,
        coordinate: None,
        max_capacity_cc: None,
        max_weight_grams: None,
        container_type_id: None,
        dimensions: None,
        weight_grams: None,
        is_fungible: None,
        fungible_unit: None,
        condition: None,
        currency: None,
        acquisition_date: None,
        acquisition_cost: None,
        current_value: None,
        depreciation_rate: None,
        warranty_expiry: None,
        system_barcode: None,
        external_codes: None,
        metadata: None,
    };

    let updated = state
        .item_commands
        .update_item(item_id, &update_req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Undo the update
    state
        .undo_commands
        .undo_event(updated.event_id, ctx.admin_id)
        .await
        .unwrap();

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    assert_eq!(item.item.name.as_deref(), Some("Original Name"));
}

#[tokio::test]
#[ignore]
async fn undo_batch_reverses_multiple_events() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create 3 items
    let mut event_ids = Vec::new();
    let mut item_ids = Vec::new();
    for i in 0..3 {
        let id = Uuid::new_v4();
        let bc = state.barcode_commands.generate_barcode().await.unwrap();
        let req = common::make_item_request(&bc.barcode, ROOT_ID, &format!("Batch {i}"), false);
        let stored = state
            .item_commands
            .create_item(id, &req, ctx.admin_id, &metadata)
            .await
            .unwrap();
        event_ids.push(stored.event_id);
        item_ids.push(id);
    }

    // Undo all 3 creations in a batch
    let results = state
        .undo_commands
        .undo_batch(&event_ids, ctx.admin_id)
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    for result in &results {
        assert_eq!(result.event_type, "ItemDeleted");
    }

    // All items should be soft-deleted
    for id in &item_ids {
        let row: (bool,) =
            sqlx::query_as("SELECT is_deleted FROM items WHERE id = $1")
                .bind(id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert!(row.0);
    }
}

#[tokio::test]
#[ignore]
async fn undo_session_reverses_full_session() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let session_id = Uuid::new_v4().to_string();
    let metadata = EventMetadata {
        session_id: Some(session_id.clone()),
        ..Default::default()
    };

    // Create 2 items with this session
    let mut item_ids = Vec::new();
    for i in 0..2 {
        let id = Uuid::new_v4();
        let bc = state.barcode_commands.generate_barcode().await.unwrap();
        let req = common::make_item_request(&bc.barcode, ROOT_ID, &format!("Session {i}"), false);
        state
            .item_commands
            .create_item(id, &req, ctx.admin_id, &metadata)
            .await
            .unwrap();
        item_ids.push(id);
    }

    // Undo the entire session
    let results = state
        .undo_commands
        .undo_session(&session_id, ctx.admin_id, 500)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);

    for id in &item_ids {
        let row: (bool,) =
            sqlx::query_as("SELECT is_deleted FROM items WHERE id = $1")
                .bind(id)
                .fetch_one(&state.pool)
                .await
                .unwrap();
        assert!(row.0);
    }
}

#[tokio::test]
#[ignore]
async fn undo_container_schema_restores_previous() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create a container and update its schema
    let container_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            container_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Schema Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let schema_event = state
        .item_commands
        .update_container_schema(
            container_id, serde_json::json!({"type": "grid", "rows": 3, "columns": 4}), std::collections::HashMap::new(), ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Undo should succeed and restore previous schema (NULL)
    let compensating = state
        .undo_commands
        .undo_event(schema_event.event_id, ctx.admin_id)
        .await
        .unwrap();
    assert_eq!(compensating.event_type, "ContainerSchemaUpdated");

    let item = state.item_queries.get_by_id(container_id).await.unwrap();
    // After undo, schema must be SQL NULL (None), not the JSON literal 'null'::jsonb
    assert_eq!(item.item.location_schema, None);
}

#[tokio::test]
#[ignore]
async fn undo_container_schema_null_not_stored_as_json_null() {
    // Regression test: undoing a ContainerSchemaUpdated whose oldSchema was None must
    // store SQL NULL in location_schema, not the JSON literal 'null'::jsonb.
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let container_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            container_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Null Schema Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Set a schema (was previously None)
    let schema_event = state
        .item_commands
        .update_container_schema(
            container_id,
            serde_json::json!({"type": "abstract", "labels": ["A"]}),
            std::collections::HashMap::new(),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Undo restores the schema to None
    state
        .undo_commands
        .undo_event(schema_event.event_id, ctx.admin_id)
        .await
        .unwrap();

    let item = state.item_queries.get_by_id(container_id).await.unwrap();
    // Must be SQL NULL (None), not Some(Value::Null) — the 'null'::jsonb literal
    assert_eq!(item.item.location_schema, None,
        "undo of initial schema set should produce SQL NULL, not JSON null literal");
}

// ── R4-B / R6-B3: undo_session with non-existent session ───────────────

#[tokio::test]
#[ignore]
async fn undo_session_nonexistent_returns_error() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let result = state
        .undo_commands
        .undo_session(&Uuid::new_v4().to_string(), ctx.admin_id, 500)
        .await;

    assert!(result.is_err());
    let msg = result.unwrap_err().to_string().to_lowercase();
    assert!(
        msg.contains("not found") || msg.contains("no undoable"),
        "unexpected error message: {msg}"
    );
}
