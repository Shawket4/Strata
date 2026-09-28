import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_documents/src/common/widgets.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// The user-owned `## Notes` section of an entity, document or place page:
/// the text as the core read it, and an inline editor whose Save replaces
/// only that section (`update_user_notes`). The AI never writes here.
class UserNotesSection extends HookConsumerWidget {
  /// Creates the section of page [id] showing [text].
  const new({
    required this.id,
    required this.text,
    super.key,
    this.title,
    this.caption,
  });

  /// The page's ID.
  final String id;

  /// The section's current text (empty when there is none).
  final String text;

  /// Heading (defaults to "Your notes").
  final String? title;

  /// Caption after the heading (defaults to "Only you edit this").
  final String? caption;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final styles = context.strataText;
    final editing = useState(false);
    final busy = useState(false);
    final controller = useTextEditingController(text: text);

    void edit() {
      controller.text = text;
      editing.value = true;
    }

    Future<void> save() async {
      if (busy.value) return;
      busy.value = true;
      final messenger = ScaffoldMessenger.maybeOf(context);
      try {
        await ref
            .read(coreApiProvider)
            .updateUserNotes(id: id, text: controller.text);
        if (context.mounted) editing.value = false;
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(l10n.userNotesSaved)));
      } on Object catch (error) {
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(
            SnackBar(
              content: Text(
                l10n.userNotesFailed(
                  code: error is CoreFailure ? error.code : 'internal',
                ),
              ),
            ),
          );
      } finally {
        if (context.mounted) busy.value = false;
      }
    }

    final Widget body;
    if (editing.value) {
      body = Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          TextField(
            controller: controller,
            autofocus: true,
            minLines: 3,
            maxLines: 12,
            keyboardType: TextInputType.multiline,
            decoration: InputDecoration(labelText: l10n.userNotesField),
          ),
          const SizedBox(height: StrataSpacing.s2),
          Wrap(
            alignment: WrapAlignment.end,
            spacing: StrataSpacing.s2,
            runSpacing: StrataSpacing.s1,
            children: [
              TextButton(
                onPressed: busy.value ? null : () => editing.value = false,
                child: Text(l10n.cancel),
              ),
              FilledButton(
                onPressed: busy.value ? null : save,
                child: Text(l10n.userNotesSave),
              ),
            ],
          ),
        ],
      );
    } else {
      body = Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(
            text.isEmpty ? l10n.userNotesEmpty : text,
            textAlign: TextAlign.start,
            style: text.isEmpty
                ? styles.bodySmall.copyWith(color: colors.text2)
                : styles.body,
          ),
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: TextButton.icon(
              onPressed: edit,
              icon: const Icon(Icons.edit_outlined, size: 18),
              label: Text(l10n.userNotesEdit),
            ),
          ),
        ],
      );
    }
    return PageSection(
      title: title ?? l10n.yourNotes,
      caption: caption ?? l10n.yourNotesHint,
      children: [body],
    );
  }
}
