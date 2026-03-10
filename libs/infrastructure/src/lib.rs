use std::sync::Mutex;

use domain::{DomainError, User, UserRepository};

#[derive(Default)]
pub struct InMemoryUserRepository {
    users: Mutex<Vec<User>>,
}

impl InMemoryUserRepository {
    pub fn new() -> Self {
        Self::default()
    }
}

impl UserRepository for InMemoryUserRepository {
    fn save(&self, user: &User) -> Result<(), DomainError> {
        let mut users = self.users.lock().map_err(|_| DomainError::Internal)?;
        users.push(user.clone());
        Ok(())
    }
}
