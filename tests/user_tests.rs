//! Integration tests for user queries (CRUD, invite tokens, refresh tokens).

mod common;

use uuid::Uuid;

#[tokio::test]
#[ignore]
async fn create_and_find_user() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let user_id = Uuid::new_v4();
    let user = state
        .user_queries
        .create(user_id, "alice", "fakehash", Some("Alice"), "member")
        .await
        .unwrap();

    assert_eq!(user.id, user_id);
    assert_eq!(user.username, "alice");

    let found = state.user_queries.find_by_id(user_id).await.unwrap();
    assert_eq!(found.username, "alice");
}

#[tokio::test]
#[ignore]
async fn username_exists_check() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    // "testadmin" was seeded
    let exists = state.user_queries.username_exists("testadmin").await.unwrap();
    assert!(exists);

    let exists = state
        .user_queries
        .username_exists("nonexistent_user_xyz")
        .await
        .unwrap();
    assert!(!exists);
}

#[tokio::test]
#[ignore]
async fn find_active_by_username() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let found = state.user_queries.find_active_by_username("testadmin").await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().id, ctx.admin_id);
}

#[tokio::test]
#[ignore]
async fn deactivate_user_hides_from_active() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let user_id = Uuid::new_v4();
    state
        .user_queries
        .create(user_id, "deactivatable", "hash", None, "member")
        .await
        .unwrap();

    state.user_queries.deactivate(user_id).await.unwrap();

    let found = state
        .user_queries
        .find_active_by_username("deactivatable")
        .await
        .unwrap();
    assert!(found.is_none());
}

#[tokio::test]
#[ignore]
async fn invite_token_lifecycle() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    // Create invite
    let invite = state.token_repository.create_invite(ctx.admin_id, 7).await.unwrap();
    assert!(!invite.code.is_empty());

    // Find it
    let found = state.token_repository.find_valid_invite(&invite.code).await.unwrap();
    assert!(found.is_some());

    // Mark used in a transaction
    let mut tx = state.pool.begin().await.unwrap();
    state
        .token_repository
        .mark_invite_used_in_tx(&mut tx, invite.id, ctx.admin_id)
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // Should no longer be valid
    let found = state.token_repository.find_valid_invite(&invite.code).await.unwrap();
    assert!(found.is_none());
}

#[tokio::test]
#[ignore]
async fn refresh_token_issue_and_revoke() {
    let ctx = common::setup().await;
    let state = &ctx.state;

    let issued = state
        .token_repository
        .issue_refresh_token(ctx.admin_id, "test-device", 30)
        .await
        .unwrap();
    assert!(!issued.raw_token.is_empty());

    // Revoke all for user
    state.token_repository.revoke_all_for_user(ctx.admin_id).await.unwrap();

    // Verify: issue a new one and revoke by hash
    let issued2 = state
        .token_repository
        .issue_refresh_token(ctx.admin_id, "test-device", 30)
        .await
        .unwrap();

    let hash = homorg::auth::jwt::hash_refresh_token(&issued2.raw_token);
    state
        .token_repository
        .revoke_by_hash(&hash, ctx.admin_id)
        .await
        .unwrap();
}
