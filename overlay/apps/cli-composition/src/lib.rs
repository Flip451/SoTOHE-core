//! CLI composition root.
//!
//! The only place allowed to know every layer at once. It constructs the
//! concrete infrastructure adapter, injects it into the usecase interactor,
//! wires that into the CLI driver, and runs the requested command — turning the
//! layered dependency graph into a single callable entry point.

use cli_driver::{CommandOutcome, GreetDriver};
use domain::Username;
use infrastructure::StaticSalutation;
use usecase::{GreetUser, UsernameError};

/// Wires the greeting feature end to end and runs it for `raw_name`.
///
/// This is the composition root: it selects the [`StaticSalutation`] adapter,
/// builds the [`GreetUser`] interactor and [`GreetDriver`], validates the input
/// into a [`Username`], and returns the rendered [`CommandOutcome`].
///
/// # Errors
///
/// Returns [`UsernameError`] when `raw_name` is empty after trimming.
pub fn run_greeting(raw_name: &str) -> Result<CommandOutcome, UsernameError> {
    let adapter = StaticSalutation::new("Hello");
    let interactor = GreetUser::new(adapter);
    let driver = GreetDriver::new(interactor);

    let user = Username::new(raw_name)?;
    Ok(driver.run(&user))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn run_greeting_wires_all_layers() {
        let outcome = run_greeting("ada").unwrap();
        assert!(outcome.success);
        assert_eq!(outcome.message, "Hello, ada!");
    }

    #[test]
    fn run_greeting_rejects_empty_name() {
        assert_eq!(run_greeting("   "), Err(UsernameError::Empty));
    }
}
