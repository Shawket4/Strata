/// Opens a note, optionally at an anchor (a block ID without `^`, or a
/// heading) that the core resolved (`EditorHint.target_anchor`,
/// `Citation.anchor`, `AskSource.anchors`). Every feature forwards note
/// navigation through this shape; the app shell routes it.
typedef OpenNoteAt = void Function(String noteId, String? anchor);
