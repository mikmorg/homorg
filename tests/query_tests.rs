//! Integration tests for query layers (items, containers, search, stats).

mod common;

use homorg::constants::ROOT_ID;
use homorg::models::event::EventMetadata;
use homorg::queries::search_queries::SearchParams;
use uuid::Uuid;

// ── Container queries ───────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn get_children_returns_direct_children() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create a parent container
    let parent_id = Uuid::new_v4();
    let bc_parent = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            parent_id,
            &common::make_item_request(&bc_parent.barcode, ROOT_ID, "Parent Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Create 3 children
    for i in 0..3 {
        let id = Uuid::new_v4();
        let bc = state.barcode_commands.generate_barcode().await.unwrap();
        state
            .item_commands
            .create_item(
                id,
                &common::make_item_request(&bc.barcode, parent_id, &format!("Child {i}"), false),
                ctx.admin_id,
                &metadata,
            )
            .await
            .unwrap();
    }

    let children = state
        .container_queries
        .get_children(parent_id, None, 50, None, None)
        .await
        .unwrap();
    assert_eq!(children.len(), 3);
}

#[tokio::test]
#[ignore]
async fn get_children_sorted_by_name() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let parent_id = Uuid::new_v4();
    let bc_parent = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            parent_id,
            &common::make_item_request(&bc_parent.barcode, ROOT_ID, "Sort Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    for name in &["Zebra", "Apple", "Mango"] {
        let id = Uuid::new_v4();
        let bc = state.barcode_commands.generate_barcode().await.unwrap();
        state
            .item_commands
            .create_item(
                id,
                &common::make_item_request(&bc.barcode, parent_id, name, false),
                ctx.admin_id,
                &metadata,
            )
            .await
            .unwrap();
    }

    let children = state
        .container_queries
        .get_children(parent_id, None, 50, Some("name"), Some("asc"))
        .await
        .unwrap();

    let names: Vec<_> = children.iter().map(|c| c.name.as_deref().unwrap_or("")).collect();
    assert_eq!(names, vec!["Apple", "Mango", "Zebra"]);
}

#[tokio::test]
#[ignore]
async fn get_children_cursor_pagination() {
    // Regression test for cursor comparison bug: the keyset cursor sub-query used
    // the outer row alias (`i.name`, `i.id`) instead of the inner cursor-row alias
    // (`i2.name`, `i2.id`), making every page-2+ query return zero results.
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let parent_id = Uuid::new_v4();
    let bc_parent = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            parent_id,
            &common::make_item_request(&bc_parent.barcode, ROOT_ID, "Cursor Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Create 5 children: Alpha, Beta, Gamma, Delta, Epsilon
    for name in &["Alpha", "Beta", "Gamma", "Delta", "Epsilon"] {
        let id = Uuid::new_v4();
        let bc = state.barcode_commands.generate_barcode().await.unwrap();
        state
            .item_commands
            .create_item(
                id,
                &common::make_item_request(&bc.barcode, parent_id, name, false),
                ctx.admin_id,
                &metadata,
            )
            .await
            .unwrap();
    }

    // Page 1: first 3 sorted by name asc
    let page1 = state
        .container_queries
        .get_children(parent_id, None, 3, Some("name"), Some("asc"))
        .await
        .unwrap();
    assert_eq!(page1.len(), 3);
    let names1: Vec<_> = page1.iter().map(|c| c.name.as_deref().unwrap_or("")).collect();
    assert_eq!(names1, vec!["Alpha", "Beta", "Delta"]);

    // Page 2: use last item's id as cursor
    let cursor = page1.last().unwrap().id;
    let page2 = state
        .container_queries
        .get_children(parent_id, Some(cursor), 3, Some("name"), Some("asc"))
        .await
        .unwrap();
    // Should return the remaining 2 items: Epsilon, Gamma
    assert_eq!(
        page2.len(),
        2,
        "page 2 must not be empty (cursor pagination regression)"
    );
    let names2: Vec<_> = page2.iter().map(|c| c.name.as_deref().unwrap_or("")).collect();
    assert_eq!(names2, vec!["Epsilon", "Gamma"]);
}

#[tokio::test]
#[ignore]
async fn get_descendants_via_ltree() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create nested containers: root -> Box -> SubBox -> item
    let box_id = Uuid::new_v4();
    let bc_box = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            box_id,
            &common::make_item_request(&bc_box.barcode, ROOT_ID, "Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let sub_box_id = Uuid::new_v4();
    let bc_sub = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            sub_box_id,
            &common::make_item_request(&bc_sub.barcode, box_id, "SubBox", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let leaf_id = Uuid::new_v4();
    let bc_leaf = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            leaf_id,
            &common::make_item_request(&bc_leaf.barcode, sub_box_id, "Leaf Item", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Get all descendants of box_id
    let descendants = state
        .container_queries
        .get_descendants(box_id, None, 100)
        .await
        .unwrap();
    assert_eq!(descendants.len(), 2); // SubBox + Leaf Item
}

#[tokio::test]
#[ignore]
async fn get_descendants_with_depth_limit() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let box_id = Uuid::new_v4();
    let bc_box = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            box_id,
            &common::make_item_request(&bc_box.barcode, ROOT_ID, "Depth Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let sub_id = Uuid::new_v4();
    let bc_sub = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            sub_id,
            &common::make_item_request(&bc_sub.barcode, box_id, "Level 1", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let deep_id = Uuid::new_v4();
    let bc_deep = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            deep_id,
            &common::make_item_request(&bc_deep.barcode, sub_id, "Level 2", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // depth=1 should return only Level 1
    let descendants = state
        .container_queries
        .get_descendants(box_id, Some(1), 100)
        .await
        .unwrap();
    assert_eq!(descendants.len(), 1);
    assert_eq!(descendants[0].name.as_deref(), Some("Level 1"));
}

#[tokio::test]
#[ignore]
async fn get_ancestors_breadcrumb() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let box_id = Uuid::new_v4();
    let bc_box = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            box_id,
            &common::make_item_request(&bc_box.barcode, ROOT_ID, "Ancestor Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let child_id = Uuid::new_v4();
    let bc_child = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            child_id,
            &common::make_item_request(&bc_child.barcode, box_id, "Deep Child", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let ancestors = state.container_queries.get_ancestors(child_id).await.unwrap();

    // Should include ROOT and Ancestor Box (at minimum)
    assert!(ancestors.len() >= 2);
    let names: Vec<_> = ancestors.iter().map(|a| a.name.as_deref().unwrap_or("")).collect();
    assert!(names.contains(&"Ancestor Box"));
}

#[tokio::test]
#[ignore]
async fn container_stats_counts() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let container_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            container_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Stats Box", true),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Add 2 children
    for i in 0..2 {
        let id = Uuid::new_v4();
        let bc = state.barcode_commands.generate_barcode().await.unwrap();
        state
            .item_commands
            .create_item(
                id,
                &common::make_item_request(&bc.barcode, container_id, &format!("Item {i}"), false),
                ctx.admin_id,
                &metadata,
            )
            .await
            .unwrap();
    }

    let stats = state.container_queries.get_stats(container_id).await.unwrap();
    assert_eq!(stats.child_count, 2);
    assert_eq!(stats.descendant_count, 2);
}

// ── Item queries ────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn get_by_barcode_returns_item() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Barcode Lookup", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let detail = state.item_queries.get_by_barcode(&bc.barcode).await.unwrap();
    assert_eq!(detail.item.id, item_id);
    assert_eq!(detail.item.name.as_deref(), Some("Barcode Lookup"));
}

#[tokio::test]
#[ignore]
async fn get_history_returns_events() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "History Item", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Delete to generate a second event
    state
        .item_commands
        .delete_item(item_id, None, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let history = state.item_queries.get_history(item_id, None, 50).await.unwrap();
    assert!(history.len() >= 2);
    assert_eq!(history[0].event_type, "ItemCreated");
}

// ── Search queries ──────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn search_fulltext_match() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    let mut req = common::make_item_request(&bc.barcode, ROOT_ID, "Quantum Oscillator Widget", false);
    req.description = Some("A highly specialized laboratory instrument".to_string());
    req.category = Some("electronics".to_string());
    state
        .item_commands
        .create_item(item_id, &req, ctx.admin_id, &metadata)
        .await
        .unwrap();

    // Full-text search for "oscillator"
    let params = SearchParams {
        q: Some("oscillator".to_string()),
        ..Default::default()
    };
    let results = state.search_queries.search(&params).await.unwrap();
    assert!(!results.is_empty());
    assert!(results.iter().any(|r| r.id == item_id));
}

#[tokio::test]
#[ignore]
async fn search_with_category_filter() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let id1 = Uuid::new_v4();
    let bc1 = state.barcode_commands.generate_barcode().await.unwrap();
    let mut req1 = common::make_item_request(&bc1.barcode, ROOT_ID, "Filtered A", false);
    req1.category = Some("tools".to_string());
    state
        .item_commands
        .create_item(id1, &req1, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let id2 = Uuid::new_v4();
    let bc2 = state.barcode_commands.generate_barcode().await.unwrap();
    let mut req2 = common::make_item_request(&bc2.barcode, ROOT_ID, "Filtered B", false);
    req2.category = Some("books".to_string());
    state
        .item_commands
        .create_item(id2, &req2, ctx.admin_id, &metadata)
        .await
        .unwrap();

    let params = SearchParams {
        category: Some("tools".to_string()),
        ..Default::default()
    };
    let results = state.search_queries.search(&params).await.unwrap();
    assert!(results.iter().all(|r| r.category.as_deref() == Some("tools")));
    assert!(results.iter().any(|r| r.id == id1));
}

#[tokio::test]
#[ignore]
async fn search_trigram_fuzzy_match() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Screwdriver Set", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    // Fuzzy/ILIKE match on partial name
    let params = SearchParams {
        q: Some("screwdriver".to_string()),
        ..Default::default()
    };
    let results = state.search_queries.search(&params).await.unwrap();
    assert!(results.iter().any(|r| r.id == item_id));
}

// ── Stats queries ───────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn system_stats_include_items_and_events() {
    let ctx = common::setup().await;
    let state = &ctx.state;
    let metadata = EventMetadata::default();

    // Create one item so we know there's at least 1
    let item_id = Uuid::new_v4();
    let bc = state.barcode_commands.generate_barcode().await.unwrap();
    state
        .item_commands
        .create_item(
            item_id,
            &common::make_item_request(&bc.barcode, ROOT_ID, "Stats Check Item", false),
            ctx.admin_id,
            &metadata,
        )
        .await
        .unwrap();

    let stats = state.stats_queries.get_stats().await.unwrap();
    assert!(stats.total_items >= 1);
    assert!(stats.total_events >= 1);
    assert!(stats.total_users >= 1);
    // ROOT, USERS, LOST are containers from seed/migration
    assert!(stats.total_containers >= 3);
}
