import 'package:flutter/material.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_ui/strata_ui.dart';

/// The formatting toolbar shown above the keyboard on phones (NoteCompact):
/// undo, heading, bold, italic, list, checklist, wikilink, mention, tag,
/// "open link" when the caret is in a resolved link, live preview on/off,
/// and hide keyboard.
///
/// Each button types markdown characters on the user's behalf; the core's
/// completions then take over for `[[`, `@` and `#`.
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

  /// Opens the wikilink under the caret (the target the core resolved).
  final OpenNoteAt? onOpenLink;

  /// Hides the on-screen keyboard.
  final VoidCallback? onHideKeyboard;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final link = controller.linkAtCaret;
    final target = link?.targetId;
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
              if (link != null && target != null)
                tool(
                  Icons.open_in_new,
                  l10n.toolOpenLink,
                  () => onOpenLink?.call(target, link.targetAnchor),
                ),
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
              separator(),
              LivePreviewToggle(controller: controller),
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

/// Live preview on / off: markdown markers hidden off the caret line, or
/// the source as typed.
class LivePreviewToggle extends StatelessWidget {
  /// Creates the toggle for [controller].
  const new({required this.controller, super.key});

  /// The editing session.
  final NoteEditorController controller;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final on = controller.livePreview;
    return IconButton(
      tooltip: on ? l10n.showMarkdown : l10n.hideMarkdown,
      isSelected: !on,
      icon: const Icon(Icons.code),
      color: colors.text,
      onPressed: () => controller.livePreview = !on,
    );
  }
}

/// The note's status line: "Unsaved changes" while the editor holds text
/// the core does not have, else the core's ready line (`NoteSyncState.label`:
/// "Saved", "Saved on this device · 1 change to sync", "Conflict"…), with
/// the icon and tone of [status].
class NoteStatusLabel extends StatelessWidget {
  /// Creates the label.
  const new({required this.status, required this.label, super.key});

  /// The status.
  final NoteEditStatus status;

  /// The core's status line.
  final String label;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final colors = context.strataColors;
    final (color, icon) = switch (status) {
      NoteEditStatus.saved => (colors.successText, Icons.check),
      NoteEditStatus.unsaved => (colors.text2, Icons.edit_outlined),
      NoteEditStatus.pending => (colors.text2, Icons.cloud_upload_outlined),
      NoteEditStatus.conflict => (colors.dangerText, Icons.error_outline),
      NoteEditStatus.duplicate => (
        colors.warningText,
        Icons.content_copy_outlined,
      ),
    };
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        Icon(icon, size: 14, color: color),
        const SizedBox(width: StrataSpacing.s1 + 2),
        Flexible(
          child: Text(
            status == NoteEditStatus.unsaved ? l10n.statusUnsaved : label,
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
                  Align(
                    alignment: AlignmentDirectional.centerEnd,
                    child: TextButton(
                      onPressed: onResolve,
                      style: TextButton.styleFrom(
                        foregroundColor: colors.warningText,
                        minimumSize: const Size(
                          64,
                          StrataLayout.minTouchTarget,
                        ),
                      ),
                      child: Text(l10n.conflictResolve),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
