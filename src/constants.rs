//! Centralised magic values and well-known identifiers.

use std::fmt;
use uuid::Uuid;

// ── Well-known seed UUIDs ───────────────────────────────────────────────

/// Root container ("Everything") — the invisible top of the LTREE.
pub const ROOT_ID: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01,
]);

/// Users container — parent of all per-user containers.
pub const USERS_ID: Uuid = Uuid::from_bytes([
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
]);

// ── LTREE / node_id constants ───────────────────────────────────────────

/// Number of hex characters from UUID.simple() used as the node_id suffix.
pub const NODE_ID_HEX_LEN: usize = 12;

/// node_id of the root container (matches migration seed).
pub const ROOT_NODE_ID: &str = "n_root";

/// node_id of the users container (matches migration seed).
pub const USERS_NODE_ID: &str = "n_users";

// ── JWT ─────────────────────────────────────────────────────────────────

/// JWT audience claim value.
pub const JWT_AUDIENCE: &str = "homorg";

// ── Password policy ─────────────────────────────────────────────────────

pub const PASSWORD_MIN_LEN: usize = 8;
pub const PASSWORD_MAX_LEN: usize = 128;

// ── Username policy ─────────────────────────────────────────────────────

pub const USERNAME_MIN_LEN: usize = 2;
pub const USERNAME_MAX_LEN: usize = 32;

/// Returns `true` if the username is well-formed (alphanumeric, underscores, hyphens).
pub fn is_valid_username(u: &str) -> bool {
    let len = u.len();
    (USERNAME_MIN_LEN..=USERNAME_MAX_LEN).contains(&len)
        && u.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && u.starts_with(|c: char| c.is_ascii_alphanumeric())
}

// ── Barcode batch limit ─────────────────────────────────────────────────

/// Maximum barcodes that can be generated in a single batch request.
pub const MAX_BARCODE_BATCH: u32 = 1000;

// ── Role hierarchy ──────────────────────────────────────────────────────

/// Typed role with an ordinal level for comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Admin,
    Member,
    Readonly,
}

impl Role {
    /// Numeric privilege level (higher = more powerful).
    pub fn level(self) -> u8 {
        match self {
            Role::Admin => 3,
            Role::Member => 2,
            Role::Readonly => 1,
        }
    }

    /// Parse a role string; unknown values map to `Readonly`.
    pub fn from_str_lossy(s: &str) -> Self {
        match s {
            "admin" => Role::Admin,
            "member" => Role::Member,
            _ => Role::Readonly,
        }
    }
}

impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Role::Admin => write!(f, "admin"),
            Role::Member => write!(f, "member"),
            Role::Readonly => write!(f, "readonly"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_levels_are_ordered() {
        assert!(Role::Admin.level() > Role::Member.level());
        assert!(Role::Member.level() > Role::Readonly.level());
    }

    #[test]
    fn role_display_roundtrip() {
        for role in [Role::Admin, Role::Member, Role::Readonly] {
            let s = role.to_string();
            assert_eq!(Role::from_str_lossy(&s), role);
        }
    }

    #[test]
    fn role_from_str_lossy_unknown_defaults_to_readonly() {
        assert_eq!(Role::from_str_lossy("superadmin"), Role::Readonly);
        assert_eq!(Role::from_str_lossy(""), Role::Readonly);
    }

    #[test]
    fn seed_ids_are_distinct() {
        assert_ne!(ROOT_ID, USERS_ID);
        assert_ne!(ROOT_ID, Uuid::nil());
        assert_ne!(USERS_ID, Uuid::nil());
    }

    #[test]
    fn node_id_hex_len_is_positive() {
        const { assert!(NODE_ID_HEX_LEN > 0) };
        // Must be ≤ 32 (UUID simple string length)
        const { assert!(NODE_ID_HEX_LEN <= 32) };
    }
}
