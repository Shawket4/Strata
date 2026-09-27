import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_home/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';

/// The capture composer: a multi-line, dictation-friendly field (typed or
/// dictated, Arabic or English) saved to the inbox with `CoreApi.capture`
/// (never refused, works offline). Ctrl/⌘+Enter saves; on desktop layouts
/// the shortcuts are shown as key hints.
class CaptureComposer extends HookConsumerWidget {
  /// Creates the composer.
  const new({
    super.key,
    this.focusNode,
    this.offline = false,
    this.minLines = 3,
  });

  /// Focus of the text field (Ctrl/⌘+N focuses it on Home).
  final FocusNode? focusNode;

  /// The device is offline (the capture is queued; the footer says so).
  final bool offline;

  /// Minimum visible lines.
  final int minLines;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final controller = useTextEditingController();
    final value = useValueListenable(controller);
    final saving = useState(false);
    final l10n = context.homeL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final desktop = SizeClass.of(context) != SizeClass.compact;

    Future<void> save() async {
      if (value.text.isEmpty || saving.value) return;
      final api = ref.read(coreApiProvider);
      final messenger = ScaffoldMessenger.maybeOf(context);
      saving.value = true;
      try {
        await api.capture(text: controller.text);
        controller.clear();
        messenger?.showSnackBar(SnackBar(content: Text(l10n.homeCaptureSaved)));
      } on Object catch (error) {
        messenger?.showSnackBar(
          SnackBar(
            content: Text(
              error is CoreFailure
                  ? l10n.homeCaptureFailedCode(code: error.code)
                  : l10n.homeCaptureFailed,
            ),
          ),
        );
      } finally {
        if (context.mounted) saving.value = false;
      }
    }

    return Semantics(
      container: true,
      label: l10n.homeComposerLabel,
      child: Card(
        margin: EdgeInsets.zero,
        child: Padding(
          padding: const EdgeInsets.all(StrataSpacing.s3),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              CallbackShortcuts(
                bindings: {
                  const SingleActivator(
                    LogicalKeyboardKey.enter,
                    control: true,
                  ): save,
                  const SingleActivator(LogicalKeyboardKey.enter, meta: true):
                      save,
                },
                child: TextField(
                  controller: controller,
                  focusNode: focusNode,
                  minLines: minLines,
                  maxLines: 10,
                  keyboardType: TextInputType.multiline,
                  textCapitalization: TextCapitalization.sentences,
                  decoration: InputDecoration(
                    labelText: l10n.homeComposerLabel,
                    hintText: l10n.homeComposerHint,
                    alignLabelWithHint: true,
                  ),
                ),
              ),
              const SizedBox(height: StrataSpacing.s2),
              Row(
                children: [
                  Icon(
                    offline ? Icons.cloud_off_outlined : Icons.mic_none,
                    size: 18,
                    color: colors.text2,
                  ),
                  const SizedBox(width: StrataSpacing.s2),
                  Expanded(
                    child: Text(
                      offline
                          ? l10n.homeComposerOffline
                          : l10n.homeComposerFooter,
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ),
                  if (desktop) ...[
                    const KeyboardHintChip(
                      keys: [KeyboardHintChip.commandKey, 'N'],
                    ),
                    const SizedBox(width: StrataSpacing.s1),
                    ExcludeSemantics(
                      child: Text(
                        l10n.homeFocusHint,
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                    ),
                    const SizedBox(width: StrataSpacing.s3),
                  ],
                  FilledButton(
                    style: tallFilledButton,
                    onPressed: value.text.isEmpty || saving.value ? null : save,
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Text(l10n.homeSave),
                        if (desktop) ...[
                          const SizedBox(width: StrataSpacing.s2),
                          const KeyboardHintChip(
                            keys: [KeyboardHintChip.commandKey, 'Enter'],
                            onAccent: true,
                          ),
                        ],
                      ],
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }
}
