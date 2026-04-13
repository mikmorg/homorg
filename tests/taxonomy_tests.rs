mod common;

use homorg::models::taxonomy::{CreateCategoryRequest, CreateTagRequest, RenameTagRequest, UpdateCategoryRequest};
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────
// Tags
// ─────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn create_tag_happy_path() {
    let ctx = common::setup().await;
    let tag = ctx
        .state
        .taxonomy_queries
        .create_tag(&CreateTagRequest {
            name: "electronics".into(),
        })
        .await
        .unwrap();
    assert_eq!(tag.name, "electronics");
    assert_eq!(tag.item_count, Some(0));

    let tags = ctx.state.taxonomy_queries.list_tags().await.unwrap();
    assert!(tags.iter().any(|t| t.name == "electronics"));
}

#[tokio::test]
#[ignore]
async fn create_tag_duplicate_returns_conflict() {
    let ctx = common::setup().await;
    ctx.state
        .taxonomy_queries
        .create_tag(&CreateTagRequest { name: "tools".into() })
        .await
        .unwrap();

    let err = ctx
        .state
        .taxonomy_queries
        .create_tag(&CreateTagRequest { name: "tools".into() })
        .await;
    assert!(err.is_err());
    let msg = format!("{:?}", err.unwrap_err());
    assert!(msg.contains("Conflict") || msg.contains("already exists"));
}

#[tokio::test]
#[ignore]
async fn get_tag_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx.state.taxonomy_queries.get_tag_by_id(Uuid::new_v4()).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn rename_tag_happy_path() {
    let ctx = common::setup().await;
    let tag = ctx
        .state
        .taxonomy_queries
        .create_tag(&CreateTagRequest {
            name: "old_name".into(),
        })
        .await
        .unwrap();

    let renamed = ctx
        .state
        .taxonomy_queries
        .rename_tag(
            tag.id,
            &RenameTagRequest {
                name: "new_name".into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(renamed.name, "new_name");
}

#[tokio::test]
#[ignore]
async fn rename_tag_to_existing_returns_conflict() {
    let ctx = common::setup().await;
    ctx.state
        .taxonomy_queries
        .create_tag(&CreateTagRequest { name: "a".into() })
        .await
        .unwrap();
    let b = ctx
        .state
        .taxonomy_queries
        .create_tag(&CreateTagRequest { name: "b".into() })
        .await
        .unwrap();

    let err = ctx
        .state
        .taxonomy_queries
        .rename_tag(b.id, &RenameTagRequest { name: "a".into() })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn rename_tag_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx
        .state
        .taxonomy_queries
        .rename_tag(Uuid::new_v4(), &RenameTagRequest { name: "x".into() })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn delete_tag_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx.state.taxonomy_queries.delete_tag(Uuid::new_v4()).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn delete_tag_happy_path() {
    let ctx = common::setup().await;
    let tag = ctx
        .state
        .taxonomy_queries
        .create_tag(&CreateTagRequest { name: "temp".into() })
        .await
        .unwrap();

    ctx.state.taxonomy_queries.delete_tag(tag.id).await.unwrap();

    let err = ctx.state.taxonomy_queries.get_tag_by_id(tag.id).await;
    assert!(err.is_err());
}

// ─────────────────────────────────────────────────────────────────────
// Categories
// ─────────────────────────────────────────────────────────────────────

#[tokio::test]
#[ignore]
async fn create_category_happy_path() {
    let ctx = common::setup().await;
    let cat = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Furniture".into(),
            description: Some("Big stuff".into()),
            parent_category_id: None,
        })
        .await
        .unwrap();
    assert_eq!(cat.name, "Furniture");
    assert_eq!(cat.item_count, Some(0));

    let cats = ctx.state.taxonomy_queries.list_categories().await.unwrap();
    assert!(cats.iter().any(|c| c.name == "Furniture"));
}

#[tokio::test]
#[ignore]
async fn create_category_duplicate_returns_conflict() {
    let ctx = common::setup().await;
    ctx.state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Tools".into(),
            description: None,
            parent_category_id: None,
        })
        .await
        .unwrap();

    let err = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Tools".into(),
            description: None,
            parent_category_id: None,
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn create_category_with_nonexistent_parent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Orphan".into(),
            description: None,
            parent_category_id: Some(Uuid::new_v4()),
        })
        .await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn get_category_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx.state.taxonomy_queries.get_category_by_id(Uuid::new_v4()).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn update_category_self_parent_returns_error() {
    let ctx = common::setup().await;
    let cat = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Self".into(),
            description: None,
            parent_category_id: None,
        })
        .await
        .unwrap();

    let err = ctx
        .state
        .taxonomy_queries
        .update_category(
            cat.id,
            &UpdateCategoryRequest {
                name: None,
                description: None,
                parent_category_id: Some(Some(cat.id)),
            },
        )
        .await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn update_category_rename() {
    let ctx = common::setup().await;
    let cat = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "OldCat".into(),
            description: None,
            parent_category_id: None,
        })
        .await
        .unwrap();

    let updated = ctx
        .state
        .taxonomy_queries
        .update_category(
            cat.id,
            &UpdateCategoryRequest {
                name: Some("NewCat".into()),
                description: None,
                parent_category_id: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.name, "NewCat");
}

#[tokio::test]
#[ignore]
async fn delete_category_nonexistent_returns_not_found() {
    let ctx = common::setup().await;
    let err = ctx.state.taxonomy_queries.delete_category(Uuid::new_v4()).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn delete_category_happy_path() {
    let ctx = common::setup().await;
    let cat = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Ephemeral".into(),
            description: None,
            parent_category_id: None,
        })
        .await
        .unwrap();

    ctx.state.taxonomy_queries.delete_category(cat.id).await.unwrap();

    let err = ctx.state.taxonomy_queries.get_category_by_id(cat.id).await;
    assert!(err.is_err());
}

#[tokio::test]
#[ignore]
async fn create_category_with_valid_parent() {
    let ctx = common::setup().await;
    let parent = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Parent".into(),
            description: None,
            parent_category_id: None,
        })
        .await
        .unwrap();

    let child = ctx
        .state
        .taxonomy_queries
        .create_category(&CreateCategoryRequest {
            name: "Child".into(),
            description: None,
            parent_category_id: Some(parent.id),
        })
        .await
        .unwrap();
    assert_eq!(child.parent_category_id, Some(parent.id));
}
