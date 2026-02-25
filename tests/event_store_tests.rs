//! Integration tests for the event store layer.

mod common;

use homorg::constants::ROOT_ID;
use homorg::models::event::{DomainEvent, EventMetadata, ItemDeletedData};
use uuid::Uuid;

#[tokio::test]
#[ignore] // Requires Docker — run with: cargo test -- --ignored
async fn append_and_replay_events() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    // Create an item first (so aggregate_id has a valid item row)
    let item_id = Uuid::new_v4();
    let barcode = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&barcode.barcode, ROOT_ID, "Event Test Item", false);
    let metadata = EventMetadata::default();
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Append a second event (delete)
    let delete_event = DomainEvent::ItemDeleted(ItemDeletedData {
        reason: Some("testing".into()),
    });
    let stored = state
        .event_store
        .append(item_id, &delete_event, ctx.admin_id, &metadata)
        .await
        .unwrap();
    assert_eq!(stored.event_type, "ItemDeleted");
    assert_eq!(stored.aggregate_id, item_id);
    assert_eq!(stored.sequence_number, 2); // create was seq 1

    // Replay all events for this aggregate
    let events = state.event_store.get_events(item_id, None).await.unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "ItemCreated");
    assert_eq!(events[0].sequence_number, 1);
    assert_eq!(events[1].event_type, "ItemDeleted");
    assert_eq!(events[1].sequence_number, 2);
}

#[tokio::test]
#[ignore]
async fn sequence_numbers_auto_increment() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let item_id = Uuid::new_v4();
    let barcode = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&barcode.barcode, ROOT_ID, "Seq Test", false);
    let metadata = EventMetadata::default();
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Append two more events
    for _ in 0..2 {
        let evt = DomainEvent::ItemDeleted(ItemDeletedData { reason: None });
        state
            .event_store
            .append(item_id, &evt, ctx.admin_id, &metadata)
            .await
            .unwrap();
    }

    let events = state.event_store.get_events(item_id, None).await.unwrap();
    assert_eq!(events.len(), 3);
    for (i, event) in events.iter().enumerate() {
        assert_eq!(event.sequence_number, (i + 1) as i64);
    }
}

#[tokio::test]
#[ignore]
async fn get_event_by_id_lookup() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let item_id = Uuid::new_v4();
    let barcode = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&barcode.barcode, ROOT_ID, "Lookup Test", false);
    let metadata = EventMetadata::default();
    let stored = state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let found = state
        .event_store
        .get_event_by_id(stored.event_id)
        .await
        .unwrap();
    assert_eq!(found.event_id, stored.event_id);
    assert_eq!(found.aggregate_id, item_id);
    assert_eq!(found.event_type, "ItemCreated");
}

#[tokio::test]
#[ignore]
async fn get_event_by_id_not_found() {
    let ctx = common::setup().await;
    let result = ctx.state.event_store.get_event_by_id(Uuid::new_v4()).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn get_events_paginated_with_filters() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    // Create two items (produces two ItemCreated events)
    for i in 0..2 {
        let id = Uuid::new_v4();
        let bc = state.barcode_commands.generate_barcode().await.unwrap();
        let req = common::make_item_request(&bc.barcode, ROOT_ID, &format!("Page {i}"), false);
        let metadata = EventMetadata::default();
        state
            .item_commands
            .create_item(id, &req, ctx.admin_id, &metadata)
            .await
            .unwrap();
    }

    // Paginate: get first page
    let page1 = state
        .event_store
        .get_events_paginated(Some("ItemCreated"), None, None, 2)
        .await
        .unwrap();
    assert!(!page1.is_empty());
    assert!(page1.len() <= 2);

    // Filter by actor_id
    let by_actor = state
        .event_store
        .get_events_paginated(None, Some(ctx.admin_id), None, 100)
        .await
        .unwrap();
    assert!(by_actor.len() >= 2);
}

#[tokio::test]
#[ignore]
async fn get_events_by_session_id() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let session_id = "test-session-001";
    let item_id = Uuid::new_v4();
    let barcode = state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&barcode.barcode, ROOT_ID, "Session Test", false);
    let metadata = EventMetadata {
        session_id: Some(session_id.to_string()),
        ..Default::default()
    };
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let events = state
        .event_store
        .get_events_by_session(session_id)
        .await
        .unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].aggregate_id, item_id);
}

#[tokio::test]
#[ignore]
async fn append_only_rejects_update() {
    let ctx = common::setup().await;

    // Try to UPDATE an event — the trigger should reject it
    let result = sqlx::query("UPDATE event_store SET event_type = 'Hacked' WHERE id = 1")
        .execute(&ctx.state.pool)
        .await;
    assert!(result.is_err(), "UPDATE on event_store should be rejected by trigger");
}

#[tokio::test]
#[ignore]
async fn append_only_rejects_delete() {
    let ctx = common::setup().await;

    let result = sqlx::query("DELETE FROM event_store WHERE id = 1")
        .execute(&ctx.state.pool)
        .await;
    assert!(result.is_err(), "DELETE on event_store should be rejected by trigger");
}
