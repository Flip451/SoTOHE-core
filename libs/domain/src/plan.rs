use crate::{TaskId, ValidationError};

/// A section within a plan, grouping related tasks under a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanSection {
    id: String,
    title: String,
    description: Vec<String>,
    task_ids: Vec<TaskId>,
}

impl PlanSection {
    /// Creates a new `PlanSection`.
    ///
    /// # Errors
    /// Returns `ValidationError::EmptyPlanSectionId` or `EmptyPlanSectionTitle` if empty.
    pub fn new(
        id: impl Into<String>,
        title: impl Into<String>,
        description: Vec<String>,
        task_ids: Vec<TaskId>,
    ) -> Result<Self, ValidationError> {
        let id = id.into();
        if id.trim().is_empty() {
            return Err(ValidationError::EmptyPlanSectionId);
        }

        let title = title.into();
        if title.trim().is_empty() {
            return Err(ValidationError::EmptyPlanSectionTitle);
        }

        Ok(Self { id, title, description, task_ids })
    }

    /// Returns the section identifier.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Returns the section title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the section description lines.
    #[must_use]
    pub fn description(&self) -> &[String] {
        &self.description
    }

    /// Returns the task IDs referenced by this section.
    #[must_use]
    pub fn task_ids(&self) -> &[TaskId] {
        &self.task_ids
    }
}

/// A read-only view of the plan: summary text and ordered sections.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlanView {
    summary: Vec<String>,
    sections: Vec<PlanSection>,
}

impl PlanView {
    /// Creates a new `PlanView`.
    #[must_use]
    pub fn new(summary: Vec<String>, sections: Vec<PlanSection>) -> Self {
        Self { summary, sections }
    }

    /// Returns the plan summary lines.
    #[must_use]
    pub fn summary(&self) -> &[String] {
        &self.summary
    }

    /// Returns the plan sections.
    #[must_use]
    pub fn sections(&self) -> &[PlanSection] {
        &self.sections
    }
}
