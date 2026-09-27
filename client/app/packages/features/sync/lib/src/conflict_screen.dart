import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/src/l10n.dart';
import 'package:strata_sync/src/labels.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// Conflict resolution (D19, SCREEN_SPEC ConflictExpanded): this device's
/// version, the editable merged result with a choice per conflicting hunk,
/// and the server's version — side by side on expanded, as tabs on compact
/// and medium. Resolving calls `CoreApi.resolveConflict`.
class ConflictResolutionScreen extends ConsumerWidget {
  /// Creates the screen for the conflicting op [opId].
  const new({required this.opId, super.key, this.onClose, this.onResolved});

  /// The conflicting op.
  final String opId;

  /// "Close and decide later".
  final VoidCallback? onClose;

  /// Called after the core accepted a resolution.
  final VoidCallback? onResolved;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.syncL10n;
    final screen = ref.watch(conflictProvider(opId));
    return switch (screen) {
      AsyncData(:final value) => switch (value.conflict) {
        final ConflictDetail detail => ConflictResolutionView(
          key: ValueKey(opId),
          opId: opId,
          detail: detail,
          onClose: onClose,
          onResolve: (resolution) async {
            await ref
                .read(coreApiProvider)
                .resolveConflict(opId: opId, resolution: resolution);
            onResolved?.call();
          },
        ),
        null => StrataEmptyState(
          icon: Icons.task_alt,
          title: l10n.conflictGone,
          message: l10n.conflictGoneBody,
        ),
      },
      AsyncError(:final error) => StrataEmptyState(
        icon: Icons.sync_problem_outlined,
        title: l10n.loadFailed,
        message: SyncLabels.failure(l10n, error),
      ),
      _ => Center(
        child: Semantics(
          label: l10n.loading,
          child: const CircularProgressIndicator(),
        ),
      ),
    };
  }
}

/// The conflict screen's content for one [ConflictDetail]; holds only the
/// ephemeral choices and the text being edited.
class ConflictResolutionView extends StatefulWidget {
  /// Creates the view.
  const new({
    required this.opId,
    required this.detail,
    required this.onResolve,
    super.key,
    this.onClose,
  });

  /// The conflicting op.
  final String opId;

  /// The conflict.
  final ConflictDetail detail;

  /// Forwards a resolution to the core.
  final Future<void> Function(ConflictResolution resolution) onResolve;

  /// Closes without deciding.
  final VoidCallback? onClose;

  @override
  State<ConflictResolutionView> createState() => _ConflictResolutionViewState();
}

class _ConflictResolutionViewState extends State<ConflictResolutionView> {
  late final TextEditingController _merged = TextEditingController(
    text: widget.detail.mergedPreview ?? '',
  );
  final Map<int, HunkChoiceKind> _choices = {};
  final Map<int, TextEditingController> _own = {};
  bool _mergedEdited = false;
  bool _busy = false;

  @override
  void dispose() {
    _merged.dispose();
    for (final controller in _own.values) {
      controller.dispose();
    }
    super.dispose();
  }

  TextEditingController _ownFor(ConflictHunkView hunk) =>
      _own.putIfAbsent(hunk.id, TextEditingController.new);

  int get _undecided => widget.detail.hunks.length - _choices.length;

  /// The resolution "Keep merged" sends: the edited text, the hunk choices,
  /// or the clean preview. `null` while a hunk is undecided.
  ConflictResolution? get _mergedResolution {
    final hunks = widget.detail.hunks;
    if (_mergedEdited || hunks.isEmpty) {
      return ConflictResolution(
        kind: ResolutionKind.merged,
        content: _merged.text,
        choices: const [],
      );
    }
    if (_undecided > 0) return null;
    return ConflictResolution(
      kind: ResolutionKind.hunks,
      choices: [
        for (final hunk in hunks)
          HunkChoice(
            hunk: hunk.id,
            choice: _choices[hunk.id]!,
            text: _choices[hunk.id] == HunkChoiceKind.text
                ? _ownFor(hunk).text
                : null,
          ),
      ],
    );
  }

  Future<void> _resolve(ConflictResolution resolution) async {
    if (_busy) return;
    setState(() => _busy = true);
    final messenger = ScaffoldMessenger.maybeOf(context);
    final l10n = context.syncL10n;
    try {
      await widget.onResolve(resolution);
    } on Object catch (error) {
      messenger?.showSnackBar(
        SnackBar(
          content: Text(
            l10n.resolveFailed(message: SyncLabels.failure(l10n, error)),
          ),
        ),
      );
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  void _keepMerged() {
    final resolution = _mergedResolution;
    if (resolution != null) unawaited(_resolve(resolution));
  }

  @override
  Widget build(BuildContext context) {
    final sizeClass = SizeClass.of(context);
    final l10n = context.syncL10n;
    final detail = widget.detail;
    final device = _VersionColumn(
      title: l10n.columnDevice,
      text: detail.local,
      semanticsLabel: l10n.columnDevice,
    );
    final server = _VersionColumn(
      title: l10n.columnServer,
      text: detail.server,
      semanticsLabel: l10n.columnServer,
    );
    final merged = _MergedColumn(
      controller: _merged,
      hunks: detail.hunks,
      choices: _choices,
      ownFor: _ownFor,
      onEdited: () => setState(() => _mergedEdited = true),
      onChoice: (hunk, choice) => setState(() => _choices[hunk] = choice),
    );
    final Widget body;
    if (sizeClass == SizeClass.expanded) {
      body = Row(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Expanded(flex: 10, child: device),
          const VerticalDivider(width: 1),
          Expanded(flex: 14, child: merged),
          const VerticalDivider(width: 1),
          Expanded(flex: 10, child: server),
        ],
      );
    } else {
      body = DefaultTabController(
        length: 3,
        initialIndex: 1,
        child: Column(
          children: [
            TabBar(
              tabs: [
                Tab(text: l10n.columnDevice),
                Tab(text: l10n.columnMerged),
                Tab(text: l10n.columnServer),
              ],
            ),
            Expanded(child: TabBarView(children: [device, merged, server])),
          ],
        ),
      );
    }
    final resolution = _mergedResolution;
    return CallbackShortcuts(
      bindings: {
        const SingleActivator(LogicalKeyboardKey.enter, meta: true):
            _keepMerged,
        const SingleActivator(LogicalKeyboardKey.enter, control: true):
            _keepMerged,
      },
      child: Focus(
        autofocus: true,
        child: Material(
          color: context.strataColors.background,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              _Bounded(
                fraction: 0.3,
                child: _Header(
                  detail: detail,
                  onClose: widget.onClose,
                  compact: sizeClass == SizeClass.compact,
                ),
              ),
              const Divider(height: 1),
              Expanded(child: body),
              const Divider(height: 1),
              _Bounded(
                fraction: 0.4,
                child: _Footer(
                  undecided: _undecided,
                  busy: _busy,
                  wide: sizeClass != SizeClass.compact,
                  onKeepServer: () => unawaited(
                    _resolve(
                      const ConflictResolution(
                        kind: ResolutionKind.keepServer,
                        choices: [],
                      ),
                    ),
                  ),
                  onKeepDevice: () => unawaited(
                    _resolve(
                      const ConflictResolution(
                        kind: ResolutionKind.keepMine,
                        choices: [],
                      ),
                    ),
                  ),
                  onKeepMerged: resolution == null ? null : _keepMerged,
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Caps [child] at [fraction] of the window height and scrolls it beyond
/// (large text scales on short windows).
class _Bounded extends StatelessWidget {
  const new({required this.fraction, required this.child});

  final double fraction;
  final Widget child;

  @override
  Widget build(BuildContext context) => ConstrainedBox(
    constraints: BoxConstraints(
      maxHeight: MediaQuery.sizeOf(context).height * fraction,
    ),
    child: SingleChildScrollView(child: child),
  );
}

class _Header extends StatelessWidget {
  const new({required this.detail, required this.compact, this.onClose});

  final ConflictDetail detail;
  final bool compact;
  final VoidCallback? onClose;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final close = onClose;
    return Padding(
      padding: EdgeInsets.fromLTRB(
        compact ? StrataSpacing.s4 : StrataSpacing.s6,
        StrataSpacing.s4,
        StrataSpacing.s3,
        StrataSpacing.s3,
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  l10n.conflictBreadcrumb,
                  style: text.caption.copyWith(color: colors.text2),
                ),
                const SizedBox(height: StrataSpacing.s1),
                Semantics(
                  header: true,
                  container: true,
                  child: Text(detail.title, style: text.title),
                ),
                const SizedBox(height: StrataSpacing.s1),
                Text(
                  detail.mergeClean ?? false
                      ? l10n.conflictCleanIntro
                      : l10n.conflictIntro,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
              ],
            ),
          ),
          if (close != null)
            IconButton(
              tooltip: l10n.decideLater,
              onPressed: close,
              icon: const Icon(Icons.close),
            ),
        ],
      ),
    );
  }
}

class _ColumnTitle extends StatelessWidget {
  const new({required this.title, this.suffix});

  final String title;
  final String? suffix;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final extra = suffix;
    return Semantics(
      header: true,
      container: true,
      child: Text.rich(
        TextSpan(
          children: [
            TextSpan(text: title),
            if (extra != null)
              TextSpan(
                text: '  $extra',
                style: text.caption.copyWith(color: colors.text2),
              ),
          ],
        ),
        style: text.bodyStrong,
      ),
    );
  }
}

class _VersionColumn extends StatelessWidget {
  const new({
    required this.title,
    required this.text,
    required this.semanticsLabel,
  });

  final String title;
  final String? text;
  final String semanticsLabel;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final styles = context.strataText;
    final content = text;
    return Semantics(
      container: true,
      label: semanticsLabel,
      explicitChildNodes: true,
      child: ListView(
        padding: const EdgeInsets.all(StrataSpacing.s4),
        children: [
          _ColumnTitle(title: title),
          const SizedBox(height: StrataSpacing.s3),
          Container(
            padding: const EdgeInsets.all(StrataSpacing.s3),
            decoration: BoxDecoration(
              color: colors.surface,
              borderRadius: StrataRadii.cardRadius,
              border: Border.all(color: colors.border),
            ),
            child: content == null
                ? Text(
                    l10n.versionMissing,
                    style: styles.bodySmall.copyWith(color: colors.text2),
                  )
                : SelectableText(
                    content,
                    textAlign: TextAlign.start,
                    style: styles.mono.copyWith(color: colors.text),
                  ),
          ),
        ],
      ),
    );
  }
}

class _MergedColumn extends StatelessWidget {
  const new({
    required this.controller,
    required this.hunks,
    required this.choices,
    required this.ownFor,
    required this.onEdited,
    required this.onChoice,
  });

  final TextEditingController controller;
  final List<ConflictHunkView> hunks;
  final Map<int, HunkChoiceKind> choices;
  final TextEditingController Function(ConflictHunkView hunk) ownFor;
  final VoidCallback onEdited;
  final void Function(int hunk, HunkChoiceKind choice) onChoice;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return SingleChildScrollView(
      padding: const EdgeInsets.all(StrataSpacing.s4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          _ColumnTitle(
            title: l10n.columnMerged,
            suffix: l10n.columnMergedEditable,
          ),
          const SizedBox(height: StrataSpacing.s3),
          for (final hunk in hunks) ...[
            _HunkCard(
              hunk: hunk,
              choice: choices[hunk.id],
              own: ownFor(hunk),
              onChoice: (choice) => onChoice(hunk.id, choice),
            ),
            const SizedBox(height: StrataSpacing.s3),
          ],
          TextField(
            controller: controller,
            onChanged: (_) => onEdited(),
            minLines: 6,
            maxLines: null,
            keyboardType: TextInputType.multiline,
            style: text.mono.copyWith(color: colors.text),
            decoration: InputDecoration(labelText: l10n.mergedFieldLabel),
          ),
        ],
      ),
    );
  }
}

class _HunkCard extends StatelessWidget {
  const new({
    required this.hunk,
    required this.choice,
    required this.own,
    required this.onChoice,
  });

  final ConflictHunkView hunk;
  final HunkChoiceKind? choice;
  final TextEditingController own;
  final ValueChanged<HunkChoiceKind> onChoice;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    // Body hunks accept the line-level choices too (sync-model
    // `HunkChoiceKind`); the kind comes from the core.
    final body = hunk.kind == 'body';
    Widget option(HunkChoiceKind kind, String label, [String? sample]) =>
        RadioListTile<HunkChoiceKind>(
          value: kind,
          contentPadding: EdgeInsets.zero,
          title: Text(label, style: text.bodySmall.withWeight(FontWeight.w600)),
          subtitle: sample == null
              ? null
              : Text(
                  sample,
                  style: text.monoSmall.copyWith(color: colors.text),
                ),
        );
    return Material(
      color: colors.warningTint,
      shape: RoundedRectangleBorder(
        borderRadius: StrataRadii.cardRadius,
        side: BorderSide(color: colors.warning.withValues(alpha: 0.4)),
      ),
      child: Padding(
        padding: const EdgeInsets.all(StrataSpacing.s3),
        child: RadioGroup<HunkChoiceKind>(
          groupValue: choice,
          onChanged: (value) {
            if (value != null) onChoice(value);
          },
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              Semantics(
                header: true,
                container: true,
                child: Text(
                  l10n.hunkTitle(location: hunk.location),
                  style: text.bodyStrong.copyWith(color: colors.warningText),
                ),
              ),
              option(HunkChoiceKind.ours, l10n.hunkOurs, hunk.ours),
              option(HunkChoiceKind.theirs, l10n.hunkTheirs, hunk.theirs),
              option(HunkChoiceKind.base, l10n.hunkBase, hunk.base),
              if (body) ...[
                option(HunkChoiceKind.oursThenTheirs, l10n.hunkBoth),
                option(HunkChoiceKind.text, l10n.hunkOwn),
                if (choice == HunkChoiceKind.text)
                  TextField(
                    controller: own,
                    minLines: 2,
                    maxLines: null,
                    style: text.mono,
                    decoration: InputDecoration(
                      labelText: l10n.hunkOwnField(location: hunk.location),
                    ),
                  ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

class _Footer extends StatelessWidget {
  const new({
    required this.undecided,
    required this.busy,
    required this.wide,
    required this.onKeepServer,
    required this.onKeepDevice,
    required this.onKeepMerged,
  });

  final int undecided;
  final bool busy;
  final bool wide;
  final VoidCallback onKeepServer;
  final VoidCallback onKeepDevice;
  final VoidCallback? onKeepMerged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.syncL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final merged = onKeepMerged;
    final buttons = [
      OutlinedButton(
        onPressed: busy ? null : onKeepServer,
        child: Text(l10n.keepServer),
      ),
      OutlinedButton(
        onPressed: busy ? null : onKeepDevice,
        child: Text(l10n.keepDevice),
      ),
      FilledButton(
        onPressed: busy || merged == null ? null : merged,
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Flexible(child: Text(l10n.keepMerged)),
            if (wide) ...[
              const SizedBox(width: StrataSpacing.s2),
              const KeyboardHintChip(
                keys: [KeyboardHintChip.commandKey, 'Enter'],
                onAccent: true,
              ),
            ],
          ],
        ),
      ),
    ];
    final summary = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          l10n.hunksLeft(count: undecided),
          style: text.bodySmall.withWeight(FontWeight.w600),
        ),
        Text(
          l10n.conflictFooter,
          style: text.caption.copyWith(color: colors.text2),
        ),
      ],
    );
    return Material(
      color: colors.surface,
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.symmetric(
            horizontal: StrataSpacing.s4,
            vertical: StrataSpacing.s3,
          ),
          child: wide
              ? Row(
                  children: [
                    Expanded(child: summary),
                    const SizedBox(width: StrataSpacing.s3),
                    Flexible(
                      flex: 2,
                      child: Wrap(
                        alignment: WrapAlignment.end,
                        spacing: StrataSpacing.s2,
                        runSpacing: StrataSpacing.s2,
                        children: buttons,
                      ),
                    ),
                  ],
                )
              : Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    summary,
                    const SizedBox(height: StrataSpacing.s2),
                    buttons[2],
                    const SizedBox(height: StrataSpacing.s2),
                    Row(
                      children: [
                        Expanded(child: buttons[0]),
                        const SizedBox(width: StrataSpacing.s2),
                        Expanded(child: buttons[1]),
                      ],
                    ),
                  ],
                ),
        ),
      ),
    );
  }
}
