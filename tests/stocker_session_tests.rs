mod common;

use common::make_item_request;
use homorg::constants::ROOT_ID;
use homorg::models::event::EventMetadata;
use homorg::models::item::MoveItemRequest;
use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn test_create_session() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let session_id = Uuid::new_v4();

    let session = state
        .session_repository
        .create(session_id, ctx.admin_id, None, None, None)
        .await
        .expect("Failed to create session");

    assert_eq!(session.id, session_id);
    assert_eq!(session.user_id, ctx.admin_id);
    assert_eq!(session.ended_at, None);
    assert_eq!(session.active_container_id, None);
    assert_eq!(session.items_scanned, 0);
    assert_eq!(session.items_created, 0);
    assert_eq!(session.items_moved, 0);
}

#[tokio::test]
#[ignore]
async fn test_list_sessions() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let session_id = Uuid::new_v4();

    // Create a session
    state
        .session_repository
        .create(session_id, ctx.admin_id, None, None, None)
        .await
        .expect("Failed to create session");

    // List sessions - should be active
    let sessions = state
        .session_repository
        .list_for_user(ctx.admin_id, 20)
        .await
        .expect("Failed to list sessions");
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].id, session_id);
    assert_eq!(sessions[0].ended_at, None);

    // End session
    state
        .session_repository
        .end_session(session_id, ctx.admin_id)
        .await
        .expect("Failed to end session");

    // List sessions - now inactive
    let sessions = state
        .session_repository
        .list_for_user(ctx.admin_id, 20)
        .await
        .expect("Failed to list sessions");
    assert_eq!(sessions.len(), 1);
    assert!(sessions[0].ended_at.is_some());
}

#[tokio::test]
#[ignore]
async fn test_get_session_for_user() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let session_id = Uuid::new_v4();
    let other_user_id = Uuid::new_v4();

    state
        .session_repository
        .create(session_id, ctx.admin_id, None, None, None)
        .await
        .expect("Failed to create session");

    // Get session as owner - should succeed
    let session = state
        .session_repository
        .get_for_user(session_id, ctx.admin_id)
        .await
        .expect("Failed to get session");
    assert_eq!(session.id, session_id);

    // Get session as different user - should fail
    let result = state
        .session_repository
        .get_for_user(session_id, other_user_id)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn test_end_session() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let session_id = Uuid::new_v4();

    state
        .session_repository
        .create(session_id, ctx.admin_id, None, None, None)
        .await
        .expect("Failed to create session");

    // End the session
    state
        .session_repository
        .end_session(session_id, ctx.admin_id)
        .await
        .expect("Failed to end session");

    // Try to get active session - should fail
    let result = state
        .session_repository
        .get_active_for_user(session_id, ctx.admin_id)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore]
async fn test_session_event_replay() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let session_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();

    state
        .session_repository
        .create(session_id, ctx.admin_id, None, None, None)
        .await
        .expect("Failed to create session");

    let metadata = EventMetadata {
        session_id: Some(session_id.to_string()),
        ..Default::default()
    };

    let req = make_item_request("TEST001", ROOT_ID, "Test Item", false);

    // Create item with session context in metadata
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .expect("Failed to create item");

    // Retrieve events for the session
    let events = state
        .event_store
        .get_events_by_session(&session_id.to_string())
        .await
        .expect("Failed to get events");

    assert!(!events.is_empty());
    assert!(events.iter().any(|e| e.aggregate_id == item_id));
}

#[tokio::test]
#[ignore]
async fn test_create_and_place_in_session() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let session_id = Uuid::new_v4();
    let container_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();

    // Create session with root container as initial
    state
        .session_repository
        .create(session_id, ctx.admin_id, None, None, Some(ROOT_ID))
        .await
        .expect("Failed to create session");

    // Create a sub-container
    let container_req = make_item_request("BOX001", ROOT_ID, "Box", true);

    state
        .item_commands
        .create_item(container_id, &container_req, ctx.admin_id, &EventMetadata::default())
        .await
        .expect("Failed to create container");

    // Create item inside the container
    let item_req = make_item_request("WIDGET001", container_id, "Widget", false);

    state
        .item_commands
        .create_item(item_id, &item_req, ctx.admin_id, &EventMetadata::default())
        .await
        .expect("Failed to create item");

    // Verify item exists in the container
    let item = state
        .item_queries
        .get_by_id(item_id)
        .await
        .expect("Failed to get item");
    assert_eq!(item.item.parent_id, Some(container_id));
    assert_eq!(item.item.name.as_deref(), Some("Widget"));
}

#[tokio::test]
#[ignore]
async fn test_move_item_in_session() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let session_id = Uuid::new_v4();
    let container_a_id = Uuid::new_v4();
    let container_b_id = Uuid::new_v4();
    let item_id = Uuid::new_v4();

    // Create session
    state
        .session_repository
        .create(session_id, ctx.admin_id, None, None, Some(ROOT_ID))
        .await
        .expect("Failed to create session");

    // Create two containers
    for (id, name) in [
        (container_a_id, "Box A"),
        (container_b_id, "Box B"),
    ] {
        let barcode = if id == container_a_id {
            "BOXA001"
        } else {
            "BOXB001"
        };
        let req = make_item_request(barcode, ROOT_ID, name, true);
        state
            .item_commands
            .create_item(id, &req, ctx.admin_id, &EventMetadata::default())
            .await
            .expect("Failed to create container");
    }

    // Create item in container A
    let item_req = make_item_request("WIDGET001", container_a_id, "Widget", false);
    state
        .item_commands
        .create_item(item_id, &item_req, ctx.admin_id, &EventMetadata::default())
        .await
        .expect("Failed to create item");

    // Move item to container B
    let move_req = MoveItemRequest {
        container_id: container_b_id,
        coordinate: None,
    };
    state
        .item_commands
        .move_item(item_id, &move_req, ctx.admin_id, &EventMetadata::default())
        .await
        .expect("Failed to move item");

    // Verify item is in container B
    let item = state
        .item_queries
        .get_by_id(item_id)
        .await
        .expect("Failed to get item");
    assert_eq!(item.item.parent_id, Some(container_b_id));
}
