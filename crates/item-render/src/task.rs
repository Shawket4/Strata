//! Task lines proposed by task suggestions (PLAN §6.11, §9.x). A task is *created* by
//! `sync_model::apply::apply_task_create` (the line, then its place in the home note).

use sync_model::suggestions::TaskPayload;
use ulid::Ulid;
use vault_format::tasks::TaskSpec;

/// The block ID of a task (§6.11): `t-<ULID in lower case>`.
pub fn task_block_id(id: Ulid) -> String {
    format!("t-{}", id.to_string().to_ascii_lowercase())
}

/// The ULID inside a task block ID of the form `t-<ulid>` (either case).
pub fn task_block_ulid(block_id: &str) -> Option<Ulid> {
    Ulid::from_string(&block_id.strip_prefix("t-")?.to_uppercase()).ok()
}

/// The description of the task an accepted suggestion creates: the proposed title followed
/// by a `[[link]]` to each entity the task concerns (`entity_links`: link texts without
/// brackets, in the payload's order) that the title does not already contain.
pub fn suggested_task_text(title: &str, entity_links: &[String]) -> String {
    let mut text = title.to_owned();
    for l in entity_links {
        let link = format!("[[{l}]]");
        if !text.contains(&link) {
            text.push(' ');
            text.push_str(&link);
        }
    }
    text
}

/// The preview of the line a task suggestion proposes: the trimmed title, the recurrence and
/// the due date in Tasks-plugin order (`- [ ] Pay rent 🔁 every month 📅 2026-10-01`).
pub fn suggestion_preview_line(p: &TaskPayload) -> String {
    TaskSpec {
        description: p.title.trim().to_owned(),
        recurrence: p.recurrence.clone(),
        due: p.due,
        ..TaskSpec::default()
    }
    .render()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use pretty_assertions::assert_eq;

    #[test]
    fn block_ids_round_trip() {
        let id = Ulid::from_string("01J8ZK3M4X7Q9W2E5R6T8Y0V1H").expect("ulid");
        assert_eq!(task_block_id(id), "t-01j8zk3m4x7q9w2e5r6t8y0v1h");
        assert_eq!(task_block_ulid("t-01j8zk3m4x7q9w2e5r6t8y0v1h"), Some(id));
        assert_eq!(task_block_ulid("t-01J8ZK3M4X7Q9W2E5R6T8Y0V1H"), Some(id));
        assert_eq!(task_block_ulid("t-x"), None);
        assert_eq!(task_block_ulid("01j8zk3m4x7q9w2e5r6t8y0v1h"), None);
    }

    #[test]
    fn suggested_text_links_each_entity_once() {
        let links = vec!["Watanya".to_owned(), "people/Shady".to_owned()];
        assert_eq!(
            suggested_task_text("Make Watanya's invoice", &links),
            "Make Watanya's invoice [[Watanya]] [[people/Shady]]"
        );
        assert_eq!(
            suggested_task_text("Call [[Watanya]]", &links[..1]),
            "Call [[Watanya]]"
        );
    }

    #[test]
    fn preview_line() {
        let p = TaskPayload {
            decision_id: Ulid::nil(),
            source_note: Ulid::nil(),
            block_id: None,
            title: "  Make Watanya's ETA invoice ".into(),
            due: NaiveDate::from_ymd_opt(2026, 10, 1),
            recurrence: Some("every month on the 1st".into()),
            reminders: Vec::new(),
            entities: Vec::new(),
            confidence: 0.9,
        };
        assert_eq!(
            suggestion_preview_line(&p),
            "- [ ] Make Watanya's ETA invoice 🔁 every month on the 1st 📅 2026-10-01"
        );
    }
}
