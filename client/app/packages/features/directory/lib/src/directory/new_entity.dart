import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// "New person" / "New company" (people and companies tabs only: the core
/// creates persons, companies and concepts).
class NewEntityButton extends StatelessWidget {
  /// Creates the button for [tab].
  const new({required this.tab, super.key, this.iconOnly = false});

  /// The directory tab.
  final DirectoryTab tab;

  /// Icon button (list pane header) instead of a labelled button.
  final bool iconOnly;

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    final String kind;
    final String label;
    switch (tab) {
      case DirectoryTab.people:
        kind = 'person';
        label = l10n.newPerson;
      case DirectoryTab.companies:
        kind = 'company';
        label = l10n.newCompany;
      case DirectoryTab.documents || DirectoryTab.places:
        return const SizedBox();
    }
    final links = EntityLinks.of(context);
    Future<void> open() => showDialog<void>(
      context: context,
      builder: (_) => DirectoryLocalizationScope(
        child: NewEntityDialog(
          kind: kind,
          title: label,
          onOpenEntity: links.onOpenEntity,
        ),
      ),
    );
    if (iconOnly) {
      return IconButton(
        tooltip: label,
        onPressed: open,
        icon: const Icon(Icons.person_add_alt),
      );
    }
    return OutlinedButton.icon(
      onPressed: open,
      icon: const Icon(Icons.add, size: 18),
      label: Text(label),
    );
  }
}

/// Creates a person or company through the core (`createEntity`); when the
/// core's duplicate check finds candidates, shows them with "Open existing"
/// and "Create anyway" (`force`).
class NewEntityDialog extends HookConsumerWidget {
  /// Creates the dialog.
  const new({
    required this.kind,
    required this.title,
    super.key,
    this.onOpenEntity,
  });

  /// `person` | `company`.
  final String kind;

  /// Dialog title.
  final String title;

  /// Opens an existing entity.
  final ValueChanged<String>? onOpenEntity;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final name = useTextEditingController();
    final alias = useTextEditingController();
    final candidates = useState<List<CandidateItem>?>(null);
    final busy = useState(false);
    final open = onOpenEntity;

    Future<void> submit({required bool force}) async {
      busy.value = true;
      final navigator = Navigator.of(context);
      final outcome = await ref
          .read(coreApiProvider)
          .createEntity(
            kind: kind,
            name: name.text,
            aliases: [if (alias.text.isNotEmpty) alias.text],
            force: force,
          );
      if (!context.mounted) return;
      busy.value = false;
      final id = outcome.id;
      if (id != null) {
        navigator.pop();
        open?.call(id);
      } else {
        candidates.value = outcome.candidates;
      }
    }

    final found = candidates.value;
    return AlertDialog(
      title: Text(title),
      content: SizedBox(
        width: 380,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              TextField(
                controller: name,
                autofocus: true,
                decoration: InputDecoration(labelText: l10n.nameField),
              ),
              const SizedBox(height: StrataSpacing.s3),
              TextField(
                controller: alias,
                decoration: InputDecoration(labelText: l10n.colAliases),
              ),
              if (found != null && found.isNotEmpty) ...[
                const SizedBox(height: StrataSpacing.s4),
                Text(
                  l10n.alreadyExists,
                  style: text.bodySmall.withWeight(FontWeight.w700),
                ),
                for (final candidate in found)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    title: Text(candidate.title),
                    subtitle: Text(
                      l10n.matchScore(
                        level: candidate.matchLevel,
                        score: candidate.score.toStringAsFixed(2),
                      ),
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                    trailing: open == null
                        ? null
                        : TextButton(
                            onPressed: () {
                              Navigator.of(context).pop();
                              open(candidate.id);
                            },
                            child: Text(l10n.openExisting),
                          ),
                  ),
              ],
            ],
          ),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        ValueListenableBuilder(
          valueListenable: name,
          builder: (context, value, _) => FilledButton(
            onPressed: value.text.isEmpty || busy.value
                ? null
                : () => submit(force: found != null && found.isNotEmpty),
            child: Text(
              found != null && found.isNotEmpty
                  ? l10n.createAnyway
                  : l10n.create,
            ),
          ),
        ),
      ],
    );
  }
}
