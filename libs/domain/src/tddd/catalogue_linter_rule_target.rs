//! Inherent implementation of [`RuleTarget`] (the type is defined in the parent
//! `catalogue_linter` module so its rustdoc path is unchanged).

use super::*;

impl RuleTarget {
    /// Creates a new `RuleTarget` from the given role list.
    ///
    /// An empty `target_roles` means "apply to all roles".
    #[must_use]
    pub fn new(target_roles: Vec<RoleKind>) -> Self {
        Self { target_roles }
    }

    /// Creates a `RuleTarget` that matches all roles.
    #[must_use]
    pub fn all_roles() -> Self {
        Self::new(vec![])
    }

    /// Returns the target roles. An empty slice means "all roles".
    #[must_use]
    pub fn target_roles(&self) -> &[RoleKind] {
        &self.target_roles
    }

    /// Returns `true` if the given `RoleKind` is in scope for this target.
    #[must_use]
    pub fn matches(&self, role: RoleKind) -> bool {
        self.target_roles.is_empty() || self.target_roles.contains(&role)
    }
}
