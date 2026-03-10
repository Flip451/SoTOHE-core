use std::process::ExitCode;

use domain::new_user;
use infrastructure::InMemoryUserRepository;
use usecase::RegisterUserUseCase;

fn main() -> ExitCode {
    let repo = InMemoryUserRepository::new();
    let usecase = RegisterUserUseCase::new(repo);

    let user = match new_user("user-1") {
        Ok(user) => user,
        Err(err) => {
            eprintln!("failed to create user: {err}");
            return ExitCode::FAILURE;
        }
    };

    if let Err(err) = usecase.execute(user) {
        eprintln!("failed to register user: {err}");
        return ExitCode::FAILURE;
    }

    println!("SoTOHE-core CLI stub");
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use domain::new_user;
    use infrastructure::InMemoryUserRepository;
    use usecase::RegisterUserUseCase;

    #[test]
    fn test_register_user_via_usecase() {
        let repo = InMemoryUserRepository::new();
        let usecase = RegisterUserUseCase::new(repo);
        let user = new_user("user-1").unwrap();
        assert!(usecase.execute(user).is_ok());
    }
}
