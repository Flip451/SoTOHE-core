use thiserror::Error;

/// Opaque user identifier. Construct via [`new_user`], which validates
/// that the underlying string is non-empty and non-whitespace-only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserId(String);

impl UserId {
    /// Returns the underlying string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: UserId,
}

impl User {
    /// Returns the user's ID as a string slice.
    #[must_use]
    pub fn id(&self) -> &str {
        self.id.as_str()
    }
}

/// Domain errors. Does not derive `PartialEq`/`Eq` so that variants can
/// wrap external error types (e.g., `sqlx::Error`) via `#[from]` in the future.
#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid user id")]
    InvalidUserId,
    #[error("internal error")]
    Internal,
}

/// Persists domain users.
///
/// # Errors
/// Returns a [`DomainError`] when the repository cannot persist `user`.
pub trait UserRepository: Send + Sync {
    fn save(&self, user: &User) -> Result<(), DomainError>;
}

/// Creates a user with the given ID.
///
/// # Errors
/// Returns [`DomainError::InvalidUserId`] if `id` is blank or whitespace-only.
pub fn new_user(id: &str) -> Result<User, DomainError> {
    if id.trim().is_empty() {
        return Err(DomainError::InvalidUserId);
    }
    Ok(User { id: UserId(id.to_owned()) })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_user_with_valid_id_returns_user() {
        let result = new_user("user-1");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().id(), "user-1");
    }

    #[test]
    fn test_new_user_with_blank_id_returns_invalid_user_id_error() {
        let result = new_user("   ");
        assert!(matches!(result, Err(DomainError::InvalidUserId)));
    }

    #[test]
    fn test_user_id_accessor_returns_original_id() {
        let user = new_user("user-2").unwrap();
        assert_eq!(user.id(), "user-2");
    }
}
