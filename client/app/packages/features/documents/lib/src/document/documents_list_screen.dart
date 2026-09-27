import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_documents/src/common/widgets.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// A stand-alone list of documents (the directory's Documents tab without
/// the other tabs), for routes that link straight to documents. Rows open
/// the document page through [onOpenEntity].
class DocumentsScreen extends StatelessWidget {
  /// Creates the list.
  const new({super.key, this.onOpenEntity});

  /// The icon that represents documents.
  static const IconData icon = Icons.description_outlined;

  /// Opens a document page (its ID).
  final ValueChanged<String>? onOpenEntity;

  @override
  Widget build(BuildContext context) => DocumentsLocalizationScope(
    child: _DocumentsList(onOpenEntity: onOpenEntity),
  );
}

class _DocumentsList extends HookConsumerWidget {
  const new({required this.onOpenEntity});

  final ValueChanged<String>? onOpenEntity;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final query = useState('');
    final async = ref.watch(
      directoryProvider(DirectoryTab.documents, query.value),
    );
    final open = onOpenEntity;
    final view = async.value;
    return ColoredBox(
      color: colors.background,
      child: ListView(
        padding: const EdgeInsets.all(StrataSpacing.s4),
        children: [
          Semantics(
            header: true,
            child: Text(l10n.documents, style: text.title),
          ),
          const SizedBox(height: StrataSpacing.s3),
          TextField(
            onChanged: (value) => query.value = value,
            decoration: InputDecoration(
              isDense: true,
              hintText: l10n.documentField,
              prefixIcon: const Icon(Icons.search, size: 20),
              fillColor: colors.surface,
            ),
          ),
          const SizedBox(height: StrataSpacing.s3),
          if (view == null && async.hasError)
            PageError(error: async.error!)
          else if (view == null)
            const PageLoading()
          else if (view.items.isEmpty)
            StrataEmptyState(
              icon: DocumentsScreen.icon,
              title: l10n.documentsCount(count: 0),
            )
          else
            Card(
              clipBehavior: Clip.antiAlias,
              child: Column(
                children: [
                  for (final item in view.items)
                    ListTile(
                      leading: const KindAvatar(kind: NodeKind.document),
                      title: Text(item.title),
                      subtitle: item.subtitle == null
                          ? null
                          : Text(item.subtitle!),
                      onTap: open == null ? null : () => open(item.id),
                    ),
                ],
              ),
            ),
        ],
      ),
    );
  }
}
