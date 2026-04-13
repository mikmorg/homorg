mod common;

use homorg::constants::ROOT_ID;
use homorg::models::event::EventMetadata;
use homorg::queries::search_queries::SearchParams;
use uuid::Uuid;

fn empty_params() -> SearchParams {
    SearchParams {
        q: None,
        path: None,
        category: None,
        condition: None,
        container_id: None,
        tags: None,
        is_container: None,
        min_value: None,
        max_value: None,
        cursor: None,
        limit: Some(50),
    }
}

#[tokio::test]
#[ignore]
async fn search_no_query_returns_results() {
    let ctx = common::setup().await;

    let bc = ctx.state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "SearchTestItem", false);
    let id = Uuid::new_v4();
    ctx.state
        .item_commands
        .create_item(id, &req, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    let results = ctx.state.search_queries.search(&empty_params()).await.unwrap();
    assert!(!results.is_empty());
}

#[tokio::test]
#[ignore]
async fn search_by_name_finds_item() {
    let ctx = common::setup().await;

    let bc = ctx.state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "UniqueSearchTarget", false);
    let id = Uuid::new_v4();
    ctx.state
        .item_commands
        .create_item(id, &req, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    let results = ctx
        .state
        .search_queries
        .search(&SearchParams {
            q: Some("UniqueSearchTarget".into()),
            ..empty_params()
        })
        .await
        .unwrap();
    assert!(results.iter().any(|r| r.id == id));
}

#[tokio::test]
#[ignore]
async fn search_deleted_items_excluded() {
    let ctx = common::setup().await;

    let bc = ctx.state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "DeletedSearchItem", false);
    let id = Uuid::new_v4();
    ctx.state
        .item_commands
        .create_item(id, &req, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    ctx.state
        .item_commands
        .delete_item(id, None, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    let results = ctx
        .state
        .search_queries
        .search(&SearchParams {
            q: Some("DeletedSearchItem".into()),
            ..empty_params()
        })
        .await
        .unwrap();
    assert!(!results.iter().any(|r| r.id == id));
}

#[tokio::test]
#[ignore]
async fn search_by_container_id() {
    let ctx = common::setup().await;

    // Create a container
    let cbc = ctx.state.barcode_commands.generate_barcode().await.unwrap();
    let creq = common::make_item_request(&cbc.barcode, ROOT_ID, "SearchContainer", true);
    let container_id = Uuid::new_v4();
    ctx.state
        .item_commands
        .create_item(container_id, &creq, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    // Create an item inside it
    let ibc = ctx.state.barcode_commands.generate_barcode().await.unwrap();
    let ireq = common::make_item_request(&ibc.barcode, container_id, "InsideItem", false);
    let item_id = Uuid::new_v4();
    ctx.state
        .item_commands
        .create_item(item_id, &ireq, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    let results = ctx
        .state
        .search_queries
        .search(&SearchParams {
            container_id: Some(container_id),
            ..empty_params()
        })
        .await
        .unwrap();
    assert!(results.iter().any(|r| r.id == item_id));
}

#[tokio::test]
#[ignore]
async fn search_with_tag_filter() {
    let ctx = common::setup().await;

    // Create an item with a tag
    let bc = ctx.state.barcode_commands.generate_barcode().await.unwrap();
    let mut req = common::make_item_request(&bc.barcode, ROOT_ID, "TaggedItem", false);
    req.tags = Some(vec!["searchable".into()]);
    let id = Uuid::new_v4();
    ctx.state
        .item_commands
        .create_item(id, &req, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    let results = ctx
        .state
        .search_queries
        .search(&SearchParams {
            tags: Some("searchable".into()),
            ..empty_params()
        })
        .await
        .unwrap();
    assert!(results.iter().any(|r| r.id == id));
}

#[tokio::test]
#[ignore]
async fn search_empty_query_does_not_error() {
    let ctx = common::setup().await;

    let results = ctx
        .state
        .search_queries
        .search(&SearchParams {
            q: Some("".into()),
            ..empty_params()
        })
        .await;
    assert!(results.is_ok());
}

#[tokio::test]
#[ignore]
async fn search_is_container_filter() {
    let ctx = common::setup().await;

    // Create a container
    let bc = ctx.state.barcode_commands.generate_barcode().await.unwrap();
    let req = common::make_item_request(&bc.barcode, ROOT_ID, "FilterableContainer", true);
    let id = Uuid::new_v4();
    ctx.state
        .item_commands
        .create_item(id, &req, ctx.admin_id, &EventMetadata::default())
        .await
        .unwrap();

    let results = ctx
        .state
        .search_queries
        .search(&SearchParams {
            q: Some("FilterableContainer".into()),
            is_container: Some(true),
            ..empty_params()
        })
        .await
        .unwrap();
    assert!(results.iter().any(|r| r.id == id));

    // Filter to non-containers should exclude it
    let results = ctx
        .state
        .search_queries
        .search(&SearchParams {
            q: Some("FilterableContainer".into()),
            is_container: Some(false),
            ..empty_params()
        })
        .await
        .unwrap();
    assert!(!results.iter().any(|r| r.id == id));
}
