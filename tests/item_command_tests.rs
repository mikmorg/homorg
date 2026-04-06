//! Integration tests for item commands (create, update, move, delete, restore, images, codes, quantity).

mod common;

use homorg::constants::ROOT_ID;
use homorg::models::event::EventMetadata;
use homorg::models::item::{AdjustQuantityRequest, MoveItemRequest, UpdateItemRequest};
use uuid::Uuid;

// ── Create ──────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn create_item_happy_path() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "Widget", false);
    let metadata = EventMetadata::default();

    let stored = state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    assert_eq!(stored.event_type, "ItemCreated");
    assert_eq!(stored.aggregate_id, item_id);
    assert_eq!(stored.sequence_number, 1);

    // Verify projection
    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    assert_eq!(item.item.name.as_deref(), Some("Widget"));
    assert!(!item.item.is_container);
    assert!(!item.item.is_deleted);
    assert_eq!(item.item.system_barcode, Some(bc.barcode));
}

#[tokio::test]
#[ignore]
async fn create_item_rejects_nonexistent_parent() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let fake_parent = Uuid::new_v4();
    let req = common::make_item_request(&bc.barcode, fake_parent, "Orphan", false);
    let metadata = EventMetadata::default();

    let result = state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn create_item_rejects_non_container_parent() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create a non-container item
    let parent_id = Uuid::new_v4();
    let bc1 = state.barcode_commands.generate_barcode().await.unwrap();
    let req1 = common::make_item_request(&bc1.barcode, ROOT_ID, "Not A Container", false);
    state
        .item_commands
        .create_item(parent_id, &req1, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Try to create a child under the non-container
    let child_id = Uuid::new_v4();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();
    let req2 = common::make_item_request(&bc2.barcode, parent_id, "Child", false);

    let result = state
        .item_commands
        .create_item(child_id, &req2, ctx.admin_id, &metadata)
        .await;

    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn create_item_rejects_duplicate_barcode() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let bc = state.barcode_commands.generate_barcode().await.unwrap();

    let id1 = Uuid::new_v4();
    let req1 = common::make_item_request(&bc.barcode, ROOT_ID, "First", false);
    state
        .item_commands
        .create_item(id1, &req1, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let id2 = Uuid::new_v4();
    let req2 = common::make_item_request(&bc.barcode, ROOT_ID, "Second", false);
    let result = state
        .item_commands
        .create_item(id2, &req2, ctx.admin_id, &metadata)
        .await;

    assert!(result.is_err());
}

// ── Update ──────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn update_item_computes_diffs() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "Before", false);
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let update_req = UpdateItemRequest {
        name: Some("After".into()),
        category: Some("electronics".into()),
        description: None,
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

    let stored = state
        .item_commands
        .update_item(item_id, &update_req, ctx.admin_id, &metadata)
        .await
        .unwrap();
    assert_eq!(stored.event_type, "ItemUpdated");

    // Verify projection
    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    assert_eq!(item.item.name.as_deref(), Some("After"));
    assert_eq!(item.item.category.as_deref(), Some("electronics"));
}

#[tokio::test]
#[ignore]
async fn update_item_no_changes_returns_error() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "NoChange", false);
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Send same name → no diff
    let update_req = UpdateItemRequest {
        name: Some("NoChange".into()),
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

    let result = state
        .item_commands
        .update_item(item_id, &update_req, ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn update_prevents_removing_container_with_children() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create a container
    let container_id = Uuid::new_v4();
    let bc1 = state.barcode_commands.generate_barcode().await.unwrap();
    let req1 = common::make_item_request(&bc1.barcode, ROOT_ID, "Box", true);
    state
        .item_commands
        .create_item(container_id, &req1, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Create a child inside it
    let child_id = Uuid::new_v4();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();
    let req2 = common::make_item_request(&bc2.barcode, container_id, "Child", false);
    state
        .item_commands
        .create_item(child_id, &req2, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Try to toggle is_container to false
    let update_req = UpdateItemRequest {
        is_container: Some(false),
        name: None,
        description: None,
        category: None,
        tags: None,
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

    let result = state
        .item_commands
        .update_item(container_id, &update_req, ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

// ── Move ────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn move_item_happy_path() {
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

    // Create an item in Box A
    let item_id = Uuid::new_v4();
    let bc_item = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc_item.barcode, box_a, "Movable", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Move to Box B
    let move_req = MoveItemRequest {
        container_id: box_b,
        coordinate: None,
    };
    let stored = state
        .item_commands
        .move_item(item_id, &move_req, ctx.admin_id, &metadata)
        .await
        .unwrap();
    assert_eq!(stored.event_type, "ItemMoved");

    // Verify projection updated
    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    assert_eq!(item.item.parent_id, Some(box_b));
}

#[tokio::test]
#[ignore]
async fn move_item_circular_reference_check() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create parent → child container hierarchy
    let parent = Uuid::new_v4();
    let bc_p = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            parent,
            &common::make_item_request(&bc_p.barcode, ROOT_ID, "Parent", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let child = Uuid::new_v4();
    let bc_c = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            child,
            &common::make_item_request(&bc_c.barcode, parent, "Child", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Try to move parent into its own child → should fail
    let move_req = MoveItemRequest {
        container_id: child,
        coordinate: None,
    };
    let result = state
        .item_commands
        .move_item(parent, &move_req, ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn move_item_rejects_non_container_destination() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let not_container = Uuid::new_v4();
    let bc1 = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            not_container,
            &common::make_item_request(&bc1.barcode, ROOT_ID, "Not Container", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let item = Uuid::new_v4();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item,
            &common::make_item_request(&bc2.barcode, ROOT_ID, "Item", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let move_req = MoveItemRequest {
        container_id: not_container,
        coordinate: None,
    };
    let result = state
        .item_commands
        .move_item(item, &move_req, ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

// ── Delete / Restore ────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn delete_item_soft_deletes() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Deletable", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let stored = state
        .item_commands
        .delete_item(item_id, Some("test reason".into()), ctx.admin_id, &metadata)
        .await
        .unwrap();
    assert_eq!(stored.event_type, "ItemDeleted");

    // Verify via raw query since get_by_id may filter deleted items
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
async fn delete_rejects_non_empty_container() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let container = Uuid::new_v4();
    let bc1 = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            container,
            &common::make_item_request(&bc1.barcode, ROOT_ID, "Full Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let child = Uuid::new_v4();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            child,
            &common::make_item_request(&bc2.barcode, container, "Inside", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let result = state
        .item_commands
        .delete_item(container, None, ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn restore_item_happy_path() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Restorable", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    state
        .item_commands
        .delete_item(item_id, None, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let stored = state
        .item_commands
        .restore_item(item_id, ctx.admin_id, &metadata)
        .await
        .unwrap();
    assert_eq!(stored.event_type, "ItemRestored");

    let row: (bool,) =
        sqlx::query_as("SELECT is_deleted FROM items WHERE id = $1")
            .bind(item_id)
            .fetch_one(&state.pool)
            .await
            .unwrap();
    assert!(!row.0);
}

// ── Image & External Code existence checks ──────────────────────────────

#[tokio::test]
#[ignore]
async fn add_image_verifies_item_exists() {
    let ctx = common::setup().await;
    let metadata = EventMetadata::default();

    let result = ctx
        .state
        .item_commands
        .add_image(
            Uuid::new_v4(), // nonexistent
            "/fake/path.jpg".into(),
            None,
            0,
            ctx.admin_id,
            &metadata,
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn remove_image_verifies_item_exists() {
    let ctx = common::setup().await;
    let metadata = EventMetadata::default();

    let result = ctx
        .state
        .item_commands
        .remove_image(Uuid::new_v4(), "/fake/path.jpg".into(), ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn add_external_code_verifies_item_exists() {
    let ctx = common::setup().await;
    let metadata = EventMetadata::default();

    let result = ctx
        .state
        .item_commands
        .add_external_code(
            Uuid::new_v4(),
            "UPC".into(),
            "123456789012".into(),
            ctx.admin_id,
            &metadata,
        )
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn remove_external_code_verifies_item_exists() {
    let ctx = common::setup().await;
    let metadata = EventMetadata::default();

    let result = ctx
        .state
        .item_commands
        .remove_external_code(
            Uuid::new_v4(),
            "UPC".into(),
            "123456789012".into(),
            ctx.admin_id,
            &metadata,
        )
        .await;
    assert!(result.is_err());
}

// ── Fungible quantity ───────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn adjust_quantity_rejects_non_fungible() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    // is_fungible defaults to false
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "Solid Item", false);
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let adj = AdjustQuantityRequest {
        new_quantity: 10,
        reason: None,
    };
    let result = state
        .item_commands
        .adjust_quantity(item_id, &adj, ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn adjust_quantity_happy_path() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let mut req = common::make_item_request(&bc.barcode, ROOT_ID, "Screws", false);
    req.is_fungible = Some(true);
    req.fungible_quantity = Some(100);
    req.fungible_unit = Some("pcs".to_string());
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let adj = AdjustQuantityRequest {
        new_quantity: 95,
        reason: Some("Used 5".into()),
    };
    let stored = state
        .item_commands
        .adjust_quantity(item_id, &adj, ctx.admin_id, &metadata)
        .await
        .unwrap();
    assert_eq!(stored.event_type, "ItemQuantityAdjusted");

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    assert_eq!(item.item.fungible_quantity, Some(95));
}

// ── Container schema ────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn update_container_schema() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let container_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "Shelf", true);
    state
        .item_commands
        .create_item(container_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let schema = serde_json::json!({
        "type": "grid",
        "rows": 3,
        "columns": 5,
    });
    let stored = state
        .item_commands
        .update_container_schema(container_id, schema.clone(), std::collections::HashMap::new(), ctx.admin_id, &metadata)
        .await
        .unwrap();
    assert_eq!(stored.event_type, "ContainerSchemaUpdated");

    let item = state.item_queries.get_by_id(container_id).await.unwrap();
    assert_eq!(item.item.location_schema, Some(schema));
}

#[tokio::test]
#[ignore]
async fn schema_label_rename_cascades_to_children() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create a container with an abstract schema
    let container_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "Box", true);
    state.item_commands.create_item(container_id, &req, ctx.admin_id, &metadata).await.unwrap();

    let schema = serde_json::json!({ "type": "abstract", "labels": ["Shelf A", "Shelf B"] });
    state.item_commands
        .update_container_schema(container_id, schema, std::collections::HashMap::new(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Add a child item positioned at "Shelf A"
    let child_id = Uuid::new_v4();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();
    let mut child_req = common::make_item_request(&bc2.barcode, container_id, "Widget", false);
    child_req.coordinate = Some(serde_json::json!({ "type": "abstract", "value": "Shelf A" }));
    state.item_commands.create_item(child_id, &child_req, ctx.admin_id, &metadata).await.unwrap();

    // Rename "Shelf A" → "Top Shelf"
    let new_schema = serde_json::json!({ "type": "abstract", "labels": ["Top Shelf", "Shelf B"] });
    let mut renames = std::collections::HashMap::new();
    renames.insert("Shelf A".to_string(), "Top Shelf".to_string());
    state.item_commands
        .update_container_schema(container_id, new_schema, renames, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Child's coordinate should now reference "Top Shelf", not "Shelf A"
    let child = state.item_queries.get_by_id(child_id).await.unwrap();
    assert_eq!(
        child.item.coordinate,
        Some(serde_json::json!({ "type": "abstract", "value": "Top Shelf" }))
    );
}

#[tokio::test]
#[ignore]
async fn schema_label_deletion_does_not_corrupt_other_children() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create container with three labels
    let container_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "Cabinet", true);
    state.item_commands.create_item(container_id, &req, ctx.admin_id, &metadata).await.unwrap();

    let schema = serde_json::json!({ "type": "abstract", "labels": ["A", "B", "C"] });
    state.item_commands
        .update_container_schema(container_id, schema, std::collections::HashMap::new(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Two children: one at "B", one at "C"
    let child_b = Uuid::new_v4();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();
    let mut req_b = common::make_item_request(&bc2.barcode, container_id, "ItemB", false);
    req_b.coordinate = Some(serde_json::json!({ "type": "abstract", "value": "B" }));
    state.item_commands.create_item(child_b, &req_b, ctx.admin_id, &metadata).await.unwrap();

    let child_c = Uuid::new_v4();
    let bc3 = state.barcode_commands.generate_barcode().await.unwrap();
    let mut req_c = common::make_item_request(&bc3.barcode, container_id, "ItemC", false);
    req_c.coordinate = Some(serde_json::json!({ "type": "abstract", "value": "C" }));
    state.item_commands.create_item(child_c, &req_c, ctx.admin_id, &metadata).await.unwrap();

    // Delete "A" from schema (no renames — pure deletion)
    let new_schema = serde_json::json!({ "type": "abstract", "labels": ["B", "C"] });
    state.item_commands
        .update_container_schema(container_id, new_schema, std::collections::HashMap::new(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Child at "B" must still be at "B", child at "C" must still be at "C"
    let b = state.item_queries.get_by_id(child_b).await.unwrap();
    assert_eq!(b.item.coordinate, Some(serde_json::json!({ "type": "abstract", "value": "B" })));

    let c = state.item_queries.get_by_id(child_c).await.unwrap();
    assert_eq!(c.item.coordinate, Some(serde_json::json!({ "type": "abstract", "value": "C" })));
}

// ── External codes ──────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn add_external_code_stores_with_uppercase_type() {
    // I5: type should be normalized to uppercase regardless of input case.
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "Coded Item", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .add_external_code(item_id, "upc".into(), "012345678905".into(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let codes: Vec<serde_json::Value> = serde_json::from_value(item.item.external_codes).unwrap();
    assert_eq!(codes.len(), 1);
    assert_eq!(codes[0]["type"].as_str().unwrap(), "UPC");
    assert_eq!(codes[0]["value"].as_str().unwrap(), "012345678905");
}

#[tokio::test]
#[ignore]
async fn add_external_code_deduplicates() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "Dup Item", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .add_external_code(item_id, "UPC".into(), "012345678905".into(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Adding the same type+value again should return an error.
    let result = state
        .item_commands
        .add_external_code(item_id, "UPC".into(), "012345678905".into(), ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());

    // Only one entry should be stored.
    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let codes: Vec<serde_json::Value> = serde_json::from_value(item.item.external_codes).unwrap();
    assert_eq!(codes.len(), 1);
}

#[tokio::test]
#[ignore]
async fn add_external_code_enforces_max() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "Many Codes", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Add exactly MAX_EXTERNAL_CODES (50) codes — all should succeed.
    for i in 0..homorg::constants::MAX_EXTERNAL_CODES {
        let value = format!("{:012}", i);
        state
            .item_commands
            .add_external_code(item_id, "EAN".into(), value, ctx.admin_id, &metadata)
            .await
            .unwrap_or_else(|e| panic!("code {i} failed: {e}"));
    }

    // The 51st should fail.
    let result = state
        .item_commands
        .add_external_code(item_id, "EAN".into(), "999999999999".into(), ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err(), "expected error when exceeding MAX_EXTERNAL_CODES");

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let codes: Vec<serde_json::Value> = serde_json::from_value(item.item.external_codes).unwrap();
    assert_eq!(codes.len(), homorg::constants::MAX_EXTERNAL_CODES);
}

#[tokio::test]
#[ignore]
async fn remove_external_code_removes_from_item() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "Remove Code", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .add_external_code(item_id, "ISBN".into(), "9780306406157".into(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .remove_external_code(item_id, "ISBN".into(), "9780306406157".into(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let codes: Vec<serde_json::Value> = serde_json::from_value(item.item.external_codes).unwrap();
    assert!(codes.is_empty());
}

#[tokio::test]
#[ignore]
async fn remove_external_code_not_found_returns_error() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "No Code", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    let result = state
        .item_commands
        .remove_external_code(item_id, "UPC".into(), "000000000000".into(), ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err());
}

// ── Images ──────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn add_image_stores_in_item() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "With Image", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .add_image(item_id, "uploads/img.jpg".into(), Some("Caption".into()), 0, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let images = item.item.images.as_array().unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["path"].as_str().unwrap(), "uploads/img.jpg");
    assert_eq!(images[0]["caption"].as_str().unwrap(), "Caption");
}

#[tokio::test]
#[ignore]
async fn remove_image_by_path_removes_from_item() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "Image Removal", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .add_image(item_id, "uploads/remove_me.jpg".into(), None, 0, ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .remove_image(item_id, "uploads/remove_me.jpg".into(), ctx.admin_id, &metadata)
        .await
        .unwrap();

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let images = item.item.images.as_array().unwrap();
    assert!(images.is_empty());
}

#[tokio::test]
#[ignore]
async fn remove_image_by_index_removes_correct_entry() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "Two Images", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    state
        .item_commands
        .add_image(item_id, "uploads/first.jpg".into(), None, 0, ctx.admin_id, &metadata)
        .await
        .unwrap();
    state
        .item_commands
        .add_image(item_id, "uploads/second.jpg".into(), None, 1, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Remove index 0 (first) — only "second.jpg" should remain.
    state
        .item_commands
        .remove_image_by_index(item_id, 0, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let images = item.item.images.as_array().unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0]["path"].as_str().unwrap(), "uploads/second.jpg");
}

#[tokio::test]
#[ignore]
async fn add_image_enforces_max_images_limit() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(item_id, &common::make_item_request(&bc.barcode, ROOT_ID, "Max Images", false), ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Add exactly MAX_IMAGES_PER_ITEM (50) images.
    for i in 0..homorg::constants::MAX_IMAGES_PER_ITEM {
        state
            .item_commands
            .add_image(item_id, format!("uploads/{i}.jpg"), None, i as i32, ctx.admin_id, &metadata)
            .await
            .unwrap_or_else(|e| panic!("image {i} failed: {e}"));
    }

    // The 51st should fail.
    let result = state
        .item_commands
        .add_image(item_id, "uploads/overflow.jpg".into(), None, 50, ctx.admin_id, &metadata)
        .await;
    assert!(result.is_err(), "expected error when exceeding MAX_IMAGES_PER_ITEM");

    let item = state.item_queries.get_by_id(item_id).await.unwrap();
    let images = item.item.images.as_array().unwrap();
    assert_eq!(images.len(), homorg::constants::MAX_IMAGES_PER_ITEM);
}
