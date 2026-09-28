import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// "New person" / "New company" / "New document" / "New place" for the tab.
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
    final (kind, label, icon) = switch (tab) {
      DirectoryTab.people => ('person', l10n.newPerson, Icons.person_add_alt),
      DirectoryTab.companies => (
        'company',
        l10n.newCompany,
        Icons.add_business_outlined,
      ),
      DirectoryTab.documents => (
        'document',
        l10n.newDocument,
        Icons.note_add_outlined,
      ),
      DirectoryTab.places => (
        'place',
        l10n.newPlace,
        Icons.add_location_alt_outlined,
      ),
    };
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
      return IconButton(tooltip: label, onPressed: open, icon: Icon(icon));
    }
    return OutlinedButton.icon(
      onPressed: open,
      icon: const Icon(Icons.add, size: 18),
      label: Text(label),
    );
  }
}

/// Creates a person or company (`create_entity`), a document
/// (`create_document`, with its type) or a place (`create_place`, with its
/// parent and address) through the core; when the core's duplicate check
/// finds candidates, shows them with "Open existing" and "Create anyway"
/// (`force`).
class NewEntityDialog extends HookConsumerWidget {
  /// Creates the dialog.
  const new({
    required this.kind,
    required this.title,
    super.key,
    this.onOpenEntity,
  });

  /// `person` | `company` | `document` | `place`.
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
    final docType = useTextEditingController();
    final address = useTextEditingController();
    final parent = useState<String?>(null);
    final places = kind == 'place'
        ? ref.watch(placeOptionsProvider(null)).value
        : null;
    final candidates = useState<List<CandidateItem>?>(null);
    final busy = useState(false);
    final open = onOpenEntity;

    Future<void> submit({required bool force}) async {
      busy.value = true;
      final navigator = Navigator.of(context);
      final core = ref.read(coreApiProvider);
      final aliases = [if (alias.text.isNotEmpty) alias.text];
      final outcome = switch (kind) {
        'document' => await core.createDocument(
          draft: DocumentDraft(
            name: name.text,
            aliases: aliases,
            docType: docType.text.isEmpty ? null : docType.text,
            companies: const [],
            people: const [],
          ),
          force: force,
        ),
        'place' => await core.createPlace(
          draft: PlaceDraft(
            name: name.text,
            aliases: aliases,
            parentId: parent.value,
            address: address.text.isEmpty ? null : address.text,
          ),
          force: force,
        ),
        _ => await core.createEntity(
          kind: kind,
          name: name.text,
          aliases: aliases,
          force: force,
        ),
      };
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
              if (kind == 'document') ...[
                const SizedBox(height: StrataSpacing.s3),
                TextField(
                  controller: docType,
                  decoration: InputDecoration(labelText: l10n.typeField),
                ),
              ],
              if (kind == 'place') ...[
                const SizedBox(height: StrataSpacing.s3),
                DropdownButtonFormField<String?>(
                  initialValue: parent.value,
                  isExpanded: true,
                  decoration: InputDecoration(labelText: l10n.parentField),
                  items: [
                    DropdownMenuItem(child: Text(l10n.noParent)),
                    for (final option in places ?? const <PlaceOption>[])
                      DropdownMenuItem(
                        value: option.id,
                        child: Padding(
                          padding: EdgeInsetsDirectional.only(
                            start: StrataSpacing.s3 * option.depth,
                          ),
                          child: Text(
                            option.title,
                            overflow: TextOverflow.ellipsis,
                          ),
                        ),
                      ),
                  ],
                  onChanged: (value) => parent.value = value,
                ),
                const SizedBox(height: StrataSpacing.s3),
                TextField(
                  controller: address,
                  decoration: InputDecoration(labelText: l10n.addressField),
                ),
              ],
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
