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

    /// Inserts a task ID into this section's task list.
    ///
    /// If `after` is `Some` and exists in the list, inserts after it.
    /// Otherwise appends to the end.
    ///
    /// This is `pub(crate)` to ensure it is only called through
    /// `TrackMetadata::add_task()` which validates invariants.
    pub(crate) fn insert_task_id(&mut self, task_id: TaskId, after: Option<&TaskId>) {
        if let Some(after_id) = after {
            if let Some(pos) = self.task_ids.iter().position(|id| id == after_id) {
                self.task_ids.insert(pos + 1, task_id);
                return;
            }
        }
        self.task_ids.push(task_id);
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

    /// Inserts a task ID into the specified section (or the first section if `section_id` is `None`).
    ///
    /// This is `pub(crate)` to ensure it is only called through
    /// `TrackMetadata::add_task()` which validates invariants.
    ///
    /// # Errors
    /// - `ValidationError::SectionNotFound` if the specified section does not exist.
    /// - `ValidationError::NoSectionsAvailable` if no sections exist and `section_id` is `None`.
    pub(crate) fn insert_task_into_section(
        &mut self,
        task_id: TaskId,
        section_id: Option<&str>,
        after_task_id: Option<&TaskId>,
    ) -> Result<(), ValidationError> {
        let target = match section_id {
            Some(sid) => self
                .sections
                .iter_mut()
                .find(|s| s.id() == sid)
                .ok_or(ValidationError::SectionNotFound(sid.to_string()))?,
            None => self.sections.first_mut().ok_or(ValidationError::NoSectionsAvailable)?,
        };
        target.insert_task_id(task_id, after_task_id);
        Ok(())
    }
}
