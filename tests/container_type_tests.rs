mod common;

use homorg::models::container_type::{CreateContainerTypeRequest, UpdateContainerTypeRequest};
use uuid::Uuid;

fn make_create(name: &str) -> CreateContainerTypeRequest {
    CreateContainerTypeRequest {
        name: name.to_string(),
        description: None,
        default_max_capacity_cc: None,
        default_max_weight_grams: None,
        default_dimensions: None,
        default_location_schema: None,
        icon: None,
        purpose: None,
    }
}

#[tokio::test]
#[ignore]
async fn create_container_type_happy_path() {
    let ctx = common::setup().await;
    let ct = ctx
        .state
        .container_type_queries
        .create(&make_create("Shelf"), ctx.admin_id)
        .await
        .unwrap();
    assert_eq!(ct.name, "Shelf");

    let fetched = ctx.state.container_type_queries.get_by_id(ct.id).await.unwrap();
    assert_eq!(fetched.name, "Shelf");
}

#[tokio::test]
#[ignore]
async fn create_container_type_with_all_fields() {
    let ctx = common::setup().await;
    let ct = ctx
        .state
        .container_type_queries
        .create(
            &CreateContainerTypeRequest {
                name: "Drawer".into(),
                description: Some("A small drawer".into()),
                default_max_capacity_cc: Some(500.0),
                default_max_weight_grams: Some(2000.0),
                default_dimensions: Some(serde_json::json!({"width_cm": 30, "height_cm": 10, "depth_cm": 40})),
                default_location_schema: Some(serde_json::json!({"type": "grid", "rows": 2, "columns": 3})),
                icon: Some("drawer".into()),
                purpose: Some("storage".into()),
            },
            ctx.admin_id,
        )
        .await
        .unwrap();
    assert_eq!(ct.name, "Drawer");
    assert_eq!(ct.icon.as_deref(), Some("drawer"));
    assert_eq!(ct.purpose.as_deref(), Some("storage"));
}

#[tokio::test]
#[ignore]
async fn get_container_type_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx.state.container_type_queries.get_by_id(Uuid::new_v4()).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn update_container_type_happy_path() {
    let ctx = common::setup().await;
    let ct = ctx
        .state
        .container_type_queries
        .create(&make_create("OldName"), ctx.admin_id)
        .await
        .unwrap();

    let updated = ctx
        .state
        .container_type_queries
        .update(
            ct.id,
            &UpdateContainerTypeRequest {
                name: Some("NewName".into()),
                description: None,
                default_max_capacity_cc: None,
                default_max_weight_grams: None,
                default_dimensions: None,
                default_location_schema: None,
                icon: None,
                purpose: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "NewName");
}

#[tokio::test]
#[ignore]
async fn update_container_type_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx
        .state
        .container_type_queries
        .update(
            Uuid::new_v4(),
            &UpdateContainerTypeRequest {
                name: Some("Ghost".into()),
                description: None,
                default_max_capacity_cc: None,
                default_max_weight_grams: None,
                default_dimensions: None,
                default_location_schema: None,
                icon: None,
                purpose: None,
            },
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn delete_container_type_happy_path() {
    let ctx = common::setup().await;
    let ct = ctx
        .state
        .container_type_queries
        .create(&make_create("Temp"), ctx.admin_id)
        .await
        .unwrap();

    ctx.state.container_type_queries.delete(ct.id).await.unwrap();

    let err = ctx.state.container_type_queries.get_by_id(ct.id).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn delete_container_type_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx.state.container_type_queries.delete(Uuid::new_v4()).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn list_container_types_returns_results() {
    let ctx = common::setup().await;
    ctx.state
        .container_type_queries
        .create(&make_create("Shelf"), ctx.admin_id)
        .await
        .unwrap();
    ctx.state
        .container_type_queries
        .create(&make_create("Bin"), ctx.admin_id)
        .await
        .unwrap();

    let types = ctx.state.container_type_queries.list_all().await.unwrap();
    assert!(types.len() >= 2);
}
