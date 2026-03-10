use domain::{DomainError, User};

pub use domain::UserRepository;

/// Application service for registering users.
pub struct RegisterUserUseCase<R: UserRepository> {
    repo: R,
}

impl<R: UserRepository> RegisterUserUseCase<R> {
    /// Creates a use case with the given repository implementation.
    pub fn new(repo: R) -> Self {
        Self { repo }
    }

    /// Persists a validated user through the configured repository.
    ///
    /// # Errors
    /// Returns any [`DomainError`] produced by the repository layer.
    pub fn execute(&self, user: User) -> Result<(), DomainError> {
        self.repo.save(&user)
    }
}
