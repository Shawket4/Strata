import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_state/strata_state.dart';

/// Builds a note screen from the note stream and its editing session.
typedef NoteSessionBuilder = Widget Function(
  BuildContext context,
  AsyncValue<NoteScreen> screen,
  NoteEditorController controller,
);

/// Subscribes to the core's view of note [noteId] (`noteProvider`), feeds
/// every view to a [NoteEditorController] and rebuilds [builder] when the
/// view or the editing state changes. Also provides the editor's strings
/// ([EditorLocalizations]) below it.
class NoteEditorSession extends ConsumerStatefulWidget {
  /// Creates the session.
  const new({required this.noteId, required this.builder, super.key});

  /// The note.
  final String noteId;

  /// Builds the screen.
  final NoteSessionBuilder builder;

  @override
  ConsumerState<NoteEditorSession> createState() => _NoteEditorSessionState();
}

class _NoteEditorSessionState extends ConsumerState<NoteEditorSession> {
  late final NoteEditorController _controller = NoteEditorController(
    core: ref.read(coreApiProvider),
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final screen = ref.watch(noteProvider(widget.noteId));
    final note = screen.value?.note;
    if (note != null && !identical(note, _controller.note)) {
      _controller.show(note);
    }
    return EditorLocalizationsScope(
      child: ListenableBuilder(
        listenable: _controller,
        builder: (context, _) => widget.builder(context, screen, _controller),
      ),
    );
  }
}

/// Makes [EditorLocalizations] available below it (the app only registers
/// the shared `StrataLocalizations`).
class EditorLocalizationsScope extends StatelessWidget {
  /// Creates the scope around [child].
  const new({required this.child, super.key});

  /// The subtree.
  final Widget child;

  @override
  Widget build(BuildContext context) => Localizations.override(
    context: context,
    delegates: const [EditorLocalizations.delegate],
    child: child,
  );
}
