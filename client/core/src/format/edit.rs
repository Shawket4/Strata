//! Content edits made by intents. The rendering of new items (captures, entity notes, task
//! lines, `id` stamps) is the shared `item-render` / `sync-model` code the server runs, so an
//! op applied locally and on the server gives the same bytes (L16); this module only maps
//! their errors.

use vault_format::tasks::TaskError;

use crate::error::CoreError;

/// Stable reason code of a task error.
pub fn task_error_code(e: &TaskError) -> &'static str {
    match e {
        TaskError::NotATask => "not_a_task",
        TaskError::NotOpen(_) => "not_open",
        TaskError::NotRecurring => "not_recurring",
        TaskError::RecurrenceNotUnderstood(_) => "recurrence_not_understood",
        TaskError::NoReferenceDate => "no_reference_date",
        TaskError::NoNextOccurrence => "no_next_occurrence",
        TaskError::InvalidBlockId(_) => "invalid_block_id",
    }
}

/// A shared rendering error as a core error (no user content in the message).
pub fn render_error(e: &item_render::RenderError) -> CoreError {
    match e {
        item_render::RenderError::Unreadable(e) => {
            CoreError::invalid("frontmatter", &e.to_string())
        }
        item_render::RenderError::Property { source, .. } => {
            CoreError::invalid("frontmatter", &source.to_string())
        }
    }
}
