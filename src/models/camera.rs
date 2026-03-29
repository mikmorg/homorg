use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// A camera token grants a remote device permission to upload images to a
/// specific stocker session without requiring full JWT authentication.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CameraToken {
    pub id: Uuid,
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub device_name: Option<String>,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
}

/// Request body for creating a camera link.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateCameraLinkRequest {
    pub device_name: Option<String>,
    /// Token lifetime in hours (default 24, max 168 = 7 days).
    pub expires_in_hours: Option<u32>,
}

/// Response returned when a camera link is created.
#[derive(Debug, Clone, Serialize)]
pub struct CameraLinkResponse {
    pub token: String,
    pub session_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub device_name: Option<String>,
}

/// Status response returned to the camera device via the token endpoint.
#[derive(Debug, Clone, Serialize)]
pub struct CameraSessionStatus {
    pub session_id: Uuid,
    pub active_container_id: Option<Uuid>,
    pub active_item_id: Option<Uuid>,
    pub session_ended: bool,
}

/// Response after a successful camera image upload.
#[derive(Debug, Clone, Serialize)]
pub struct CameraUploadResponse {
    pub item_id: Uuid,
    pub image_url: String,
    pub image_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserialize_create_camera_link_empty() {
        let r: CreateCameraLinkRequest = serde_json::from_str("{}").unwrap();
        assert!(r.device_name.is_none());
        assert!(r.expires_in_hours.is_none());
    }

    #[test]
    fn deserialize_create_camera_link_full() {
        let json = r#"{"device_name":"Galaxy S24","expires_in_hours":48}"#;
        let r: CreateCameraLinkRequest = serde_json::from_str(json).unwrap();
        assert_eq!(r.device_name.as_deref(), Some("Galaxy S24"));
        assert_eq!(r.expires_in_hours, Some(48));
    }

    #[test]
    fn serialize_camera_link_response() {
        let r = CameraLinkResponse {
            token: "abc123".into(),
            session_id: Uuid::nil(),
            expires_at: DateTime::<Utc>::from_timestamp(0, 0).unwrap(),
            device_name: Some("test".into()),
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["token"], "abc123");
        assert!(v["session_id"].is_string());
        assert!(v["expires_at"].is_string());
    }

    #[test]
    fn serialize_camera_session_status() {
        let s = CameraSessionStatus {
            session_id: Uuid::nil(),
            active_container_id: None,
            active_item_id: Some(Uuid::nil()),
            session_ended: false,
        };
        let v: serde_json::Value = serde_json::to_value(&s).unwrap();
        assert_eq!(v["session_ended"], false);
        assert!(v["active_item_id"].is_string());
    }

    #[test]
    fn serialize_camera_upload_response() {
        let r = CameraUploadResponse {
            item_id: Uuid::nil(),
            image_url: "/files/test.jpg".into(),
            image_count: 3,
        };
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["image_count"], 3);
        assert_eq!(v["image_url"], "/files/test.jpg");
    }
}
