import 'package:flutter/material.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_ui/strata_ui.dart';

/// The formatting toolbar shown above the keyboard on phones (NoteCompact):
/// undo, heading, bold, italic, list, checklist, wikilink, mention, tag,
/// "open link" when the caret is in a link, and hide keyboard.
///
/// Each button types markdown characters on the user's behalf; autocomplete
/// then takes over for `[[`, `@` and `#`.
class FormattingToolbar extends StatelessWidget {
  /// Creates the toolbar for [controller].
  const new({
    required this.controller,
    super.key,
    this.onOpenLink,
    this.onHideKeyboard,
  });

  /// The editing session.
  final NoteEditorController controller;

  /// Opens the wikilink under the caret.
  final ValueChanged<String>? onOpenLink;

  /// Hides the on-screen keyboard.
  final VoidCallback? onHideKeyboard;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final link = controller.linkAtCaret;
    Widget tool(IconData icon, String label, VoidCallback? onPressed) =>
        IconButton(
          tooltip: label,
          icon: Icon(icon),
          color: colors.text,
          onPressed: onPressed,
        );
    Widget separator() => Container(
      width: 1,
      height: 22,
      margin: const EdgeInsets.symmetric(horizontal: StrataSpacing.s1),
      color: colors.border,
    );
    return Semantics(
      container: true,
      label: l10n.toolbarLabel,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: colors.surface,
          border: Border(top: BorderSide(color: colors.border)),
        ),
        child: SingleChildScrollView(
          scrollDirection: Axis.horizontal,
          padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s1),
          child: Row(
            children: [
              tool(Icons.undo, l10n.toolUndo, controller.undo),
              separator(),
              tool(
                Icons.title,
                l10n.toolHeading,
                () => controller.prefixLine('# '),
              ),
              tool(
                Icons.format_bold,
                l10n.toolBold,
                () => controller.wrapSelection('**'),
              ),
              tool(
                Icons.format_italic,
                l10n.toolItalic,
                () => controller.wrapSelection('_'),
              ),
              tool(
                Icons.format_list_bulleted,
                l10n.toolBulletList,
                () => controller.prefixLine('- '),
              ),
              tool(
                Icons.check_box_outlined,
                l10n.toolChecklist,
                () => controller.prefixLine('- [ ] '),
              ),
              tool(
                Icons.link,
                l10n.toolWikilink,
                () => controller.insertAtCaret('[['),
              ),
              tool(
                Icons.alternate_email,
                l10n.toolMention,
                () => controller.insertAtCaret('@'),
              ),
              tool(
                Icons.tag,
                l10n.toolTag,
                () => controller.insertAtCaret('#'),
              ),
              if (link != null)
                tool(
                  Icons.open_in_new,
                  l10n.toolOpenLink,
                  () => onOpenLink?.call(link),
                ),
              separator(),
              tool(
                Icons.keyboard_hide_outlined,
                l10n.toolHideKeyboard,
                onHideKeyboard,
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// The note's status line ("Saved", "Unsaved changes", "Saved on this
/// device · 1 change to sync", "Conflict"), mapped 1:1 from
/// [NoteEditStatus].
class NoteStatusLabel extends StatelessWidget {
  /// Creates the label.
  const new({required this.status, required this.pendingOps, super.key});

  /// The status.
  final NoteEditStatus status;

  /// Ops waiting to sync (from the note's sync state).
  final int pendingOps;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final (label, color, icon) = switch (status) {
      NoteEditStatus.saved => (
        l10n.statusSaved,
        colors.successText,
        Icons.check,
      ),
      NoteEditStatus.unsaved => (
        l10n.statusUnsaved,
        colors.text2,
        Icons.edit_outlined,
      ),
      NoteEditStatus.pending => (
        l10n.statusPending(count: pendingOps),
        colors.text2,
        Icons.cloud_upload_outlined,
      ),
      NoteEditStatus.conflict => (
        l10n.statusConflict,
        colors.dangerText,
        Icons.error_outline,
      ),
    };
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 14, color: color),
        const SizedBox(width: StrataSpacing.s1 + 2),
        Flexible(
          child: Text(
            label,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: context.strataText.caption.copyWith(color: color),
          ),
        ),
      ],
    );
  }
}

/// The 409 hand-off: shown while the note's edit conflicts with the server;
/// "Resolve" opens the sync package's conflict screen for the conflicting
/// op.
class NoteConflictBanner extends StatelessWidget {
  /// Creates the banner.
  const new({required this.onResolve, super.key});

  /// Opens the conflict screen.
  final VoidCallback? onResolve;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final text = context.strataText;
    return Semantics(
      container: true,
      liveRegion: true,
      child: Container(
        padding: const EdgeInsetsDirectional.fromSTEB(
          StrataSpacing.s4,
          StrataSpacing.s3,
          StrataSpacing.s2,
          StrataSpacing.s3,
        ),
        decoration: BoxDecoration(
          color: colors.warningTint,
          border: BorderDirectional(
            start: BorderSide(color: colors.warning, width: 3),
          ),
          borderRadius: StrataRadii.inputRadius,
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.only(top: 2),
              child: Icon(
                Icons.call_split,
                size: 20,
                color: colors.warningText,
              ),
            ),
            const SizedBox(width: StrataSpacing.s3),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    l10n.conflictBannerTitle,
                    style: text.bodySmall.copyWith(
                      color: colors.warningText,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  Text(
                    l10n.conflictBannerMessage,
                    style: text.caption.copyWith(color: colors.warningText),
                  ),
                ],
              ),
            ),
            const SizedBox(width: StrataSpacing.s2),
            TextButton(
              onPressed: onResolve,
              style: TextButton.styleFrom(
                foregroundColor: colors.warningText,
                minimumSize: const Size(64, StrataLayout.minTouchTarget),
              ),
              child: Text(l10n.conflictResolve),
            ),
          ],
        ),
      ),
    );
  }
}
