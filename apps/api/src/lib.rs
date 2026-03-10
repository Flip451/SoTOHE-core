use usecase::{RegisterUserUseCase, UserRepository};

/// Registers application routes for user registration.
pub fn route_registration<R: UserRepository>(_usecase: &RegisterUserUseCase<R>) -> &'static str {
    "/register"
}
