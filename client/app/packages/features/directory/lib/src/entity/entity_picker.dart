import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_directory/src/entity/sections.dart';
import 'package:strata_documents/strata_documents.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Asks for an entity of [tab] (merge target, repoint target) and returns
/// its ID, or null when cancelled. [excludeId] (the entity itself) is shown
/// but cannot be chosen.
Future<String?> showEntityPicker(
  BuildContext context, {
  required DirectoryTab tab,
  required String title,
  String? excludeId,
}) => showDialog<String>(
  context: context,
  builder: (_) => DirectoryLocalizationScope(
    child: DocumentsLocalizationScope(
      child: EntityPickerDialog(tab: tab, title: title, excludeId: excludeId),
    ),
  ),
);

/// A searchable list of one directory tab (the core matches names and
/// aliases in both scripts); choosing a row closes the dialog with its ID.
class EntityPickerDialog extends HookConsumerWidget {
  /// Creates the picker.
  const new({
    required this.tab,
    required this.title,
    super.key,
    this.excludeId,
  });

  /// The tab to pick from.
  final DirectoryTab tab;

  /// Dialog title.
  final String title;

  /// An ID that cannot be chosen.
  final String? excludeId;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.dirL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final query = useState('');
    final view = ref.watch(directoryProvider(tab, query.value)).value;
    return AlertDialog(
      title: Text(title),
      content: SizedBox(
        width: 420,
        height: 360,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              autofocus: true,
              onChanged: (value) => query.value = value,
              decoration: InputDecoration(
                isDense: true,
                labelText: l10n.pickerSearch,
                hintText: l10n.searchHint,
                prefixIcon: const Icon(Icons.search, size: 20),
              ),
            ),
            const SizedBox(height: StrataSpacing.s2),
            Expanded(
              child: view == null
                  ? const PageLoading()
                  : view.items.isEmpty
                  ? Center(
                      child: Text(
                        l10n.noMatches(query: view.query),
                        style: text.bodySmall.copyWith(color: colors.text2),
                      ),
                    )
                  : ListView(
                      children: [
                        for (final item in view.items)
                          ListTile(
                            enabled: item.id != excludeId,
                            leading: EntityAvatar(
                              kind: tabKind(tab),
                              initials: item.initials,
                              size: 32,
                            ),
                            title: Text(
                              item.title,
                              textDirection: textDirectionOf(item.titleDir),
                              textAlign: TextAlign.start,
                            ),
                            subtitle: item.subtitle == null
                                ? null
                                : Text(item.subtitle!),
                            onTap: () => Navigator.of(context).pop(item.id),
                          ),
                      ],
                    ),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
      ],
    );
  }
}
