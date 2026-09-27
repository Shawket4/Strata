import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// The custody event types offered by "Record a move" (PLAN §6.12), with
/// their vault names.
enum MoveEvent {
  /// `moved-to`.
  movedTo('moved-to'),

  /// `handed-to`.
  handedTo('handed-to'),

  /// `returned-by`.
  returned('returned-by'),

  /// `sent-to` (third party).
  sentTo('sent-to'),

  /// `lost`.
  lost('lost');

  new(this.wire);

  /// The custody event type as written in `## Custody`.
  final String wire;

  /// Label.
  String label(DocumentsLocalizations l10n) => switch (this) {
    MoveEvent.movedTo => l10n.eventMovedTo,
    MoveEvent.handedTo => l10n.eventHandedTo,
    MoveEvent.returned => l10n.eventReturned,
    MoveEvent.sentTo => l10n.eventSentTo,
    MoveEvent.lost => l10n.eventLost,
  };
}

/// Opens "Record a move" for [document] (or, from a place page, a choice of
/// [documents]): a bottom sheet on compact, a dialog otherwise.
Future<void> showRecordMove(
  BuildContext context, {
  EntityRef? document,
  List<EntityRef> location = const [],
  List<DocumentBrief> documents = const [],
}) {
  final form = DocumentsLocalizationScope(
    child: RecordMoveForm(
      document: document,
      location: location,
      documents: documents,
    ),
  );
  if (SizeClass.of(context) == SizeClass.compact) {
    return showModalBottomSheet<void>(
      context: context,
      isScrollControlled: true,
      useSafeArea: true,
      builder: (_) => FractionallySizedBox(heightFactor: 0.92, child: form),
    );
  }
  return showDialog<void>(
    context: context,
    builder: (_) => Dialog(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460, maxHeight: 720),
        child: form,
      ),
    ),
  );
}

/// The "Record a move" form: what happened, to which place (nested places
/// from the directory, with their parent), the holder after, the date and a
/// note. Recording needs a custody intent in the core, which does not exist
/// yet: the form is complete but its submit button is disabled with a notice.
class RecordMoveForm extends HookConsumerWidget {
  /// Creates the form.
  const new({
    super.key,
    this.document,
    this.location = const [],
    this.documents = const [],
  });

  /// The document being moved (document page).
  final EntityRef? document;

  /// Its current location breadcrumb.
  final List<EntityRef> location;

  /// Documents to choose from (place page).
  final List<DocumentBrief> documents;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final event = useState(MoveEvent.movedTo);
    final place = useState<String?>(null);
    final holder = useState<String?>(null);
    final chosenDocument = useState<String?>(document?.id);
    final date = useState<DateTime?>(null);
    final placeQuery = useState('');
    final personQuery = useState('');
    final note = useTextEditingController();
    final places = ref
        .watch(directoryProvider(DirectoryTab.places, placeQuery.value))
        .value
        ?.items;
    final people = ref
        .watch(directoryProvider(DirectoryTab.people, personQuery.value))
        .value
        ?.items;
    final label = text.caption
        .withWeight(FontWeight.w700)
        .copyWith(color: colors.text2);
    final doc = document;
    final here = location.isEmpty ? null : location.last;
    Widget field(String name, Widget child) => Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s4),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(name, style: label),
          const SizedBox(height: StrataSpacing.s1),
          child,
        ],
      ),
    );

    return Material(
      type: MaterialType.transparency,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsetsDirectional.fromSTEB(
              StrataSpacing.s5,
              StrataSpacing.s4,
              StrataSpacing.s2,
              0,
            ),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Semantics(
                        header: true,
                        child: Text(l10n.recordMove, style: text.titleSmall),
                      ),
                      if (doc != null)
                        Text(
                          here == null
                              ? l10n.recordMoveSubtitleUnknown(
                                  document: doc.title,
                                )
                              : l10n.recordMoveSubtitle(
                                  document: doc.title,
                                  place: here.title,
                                ),
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                    ],
                  ),
                ),
                IconButton(
                  tooltip: l10n.close,
                  onPressed: () => Navigator.of(context).maybePop(),
                  icon: const Icon(Icons.close),
                ),
              ],
            ),
          ),
          Expanded(
            child: ListView(
              padding: const EdgeInsets.fromLTRB(
                StrataSpacing.s5,
                0,
                StrataSpacing.s5,
                StrataSpacing.s4,
              ),
              children: [
                if (doc == null)
                  field(
                    l10n.documentField,
                    RadioGroup<String?>(
                      groupValue: chosenDocument.value,
                      onChanged: (value) => chosenDocument.value = value,
                      child: Column(
                        children: [
                          for (final item in documents)
                            RadioListTile<String?>(
                              dense: true,
                              value: item.id,
                              title: Text(item.title),
                            ),
                        ],
                      ),
                    ),
                  ),
                field(
                  l10n.whatHappened,
                  Wrap(
                    spacing: StrataSpacing.s2,
                    runSpacing: StrataSpacing.s1,
                    children: [
                      for (final value in MoveEvent.values)
                        ChoiceChip(
                          label: Text(value.label(l10n)),
                          selected: event.value == value,
                          onSelected: (_) => event.value = value,
                        ),
                    ],
                  ),
                ),
                field(
                  l10n.toPlace,
                  _Picker(
                    searchLabel: l10n.searchPlaces,
                    onQuery: (value) => placeQuery.value = value,
                    selected: place.value,
                    onSelected: (value) => place.value = value,
                    items: places,
                    icon: NodeKind.place,
                  ),
                ),
                field(
                  l10n.holderAfter,
                  _Picker(
                    searchLabel: l10n.searchPeople,
                    onQuery: (value) => personQuery.value = value,
                    selected: holder.value,
                    onSelected: (value) => holder.value = value,
                    items: people,
                    icon: NodeKind.person,
                    noneLabel: l10n.nobody,
                  ),
                ),
                field(
                  l10n.dateField,
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: OutlinedButton.icon(
                      icon: const Icon(Icons.event_outlined, size: 18),
                      label: Text(
                        date.value == null
                            ? l10n.dateToday
                            : l10n.dateShort(date: date.value!),
                      ),
                      onPressed: () async {
                        final picked = await showDatePicker(
                          context: context,
                          initialDate: date.value ?? DateTime.now(),
                          firstDate: DateTime(1990),
                          lastDate: DateTime(2100),
                        );
                        if (picked != null) date.value = picked;
                      },
                    ),
                  ),
                ),
                field(
                  l10n.noteField,
                  TextField(controller: note, minLines: 2, maxLines: 4),
                ),
              ],
            ),
          ),
          Divider(height: 1, color: colors.border),
          Padding(
            padding: const EdgeInsets.fromLTRB(
              StrataSpacing.s5,
              StrataSpacing.s3,
              StrataSpacing.s5,
              StrataSpacing.s3,
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Text(
                  l10n.recordMoveFooter,
                  style: text.caption.copyWith(color: colors.text2),
                ),
                const SizedBox(height: StrataSpacing.s1),
                Row(
                  children: [
                    Icon(Icons.info_outline, size: 16, color: colors.infoText),
                    const SizedBox(width: StrataSpacing.s1),
                    Expanded(
                      child: Text(
                        l10n.recordMoveUnavailable,
                        style: text.caption.copyWith(color: colors.infoText),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: StrataSpacing.s2),
                Wrap(
                  alignment: WrapAlignment.end,
                  spacing: StrataSpacing.s2,
                  runSpacing: StrataSpacing.s2,
                  children: [
                    OutlinedButton(
                      onPressed: () => Navigator.of(context).maybePop(),
                      style: OutlinedButton.styleFrom(
                        minimumSize: const Size(48, 48),
                      ),
                      child: Text(l10n.cancel),
                    ),
                    FilledButton(
                      onPressed: null,
                      style: FilledButton.styleFrom(
                        minimumSize: const Size(48, 48),
                        disabledForegroundColor: colors.text2,
                        disabledBackgroundColor: colors.surface2,
                      ),
                      child: Text(l10n.recordMoveSubmit),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _Picker extends HookWidget {
  const new({
    required this.searchLabel,
    required this.onQuery,
    required this.selected,
    required this.onSelected,
    required this.items,
    required this.icon,
    this.noneLabel,
  });

  final String searchLabel;
  final ValueChanged<String> onQuery;
  final String? selected;
  final ValueChanged<String?> onSelected;
  final List<DirectoryItem>? items;
  final NodeKind icon;
  final String? noneLabel;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final none = noneLabel;
    final list = items;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        TextField(
          onChanged: onQuery,
          decoration: InputDecoration(
            isDense: true,
            hintText: searchLabel,
            prefixIcon: const Icon(Icons.search, size: 20),
          ),
        ),
        const SizedBox(height: StrataSpacing.s1),
        DecoratedBox(
          decoration: BoxDecoration(
            border: Border.all(color: colors.border),
            borderRadius: StrataRadii.inputRadius,
          ),
          child: RadioGroup<String?>(
            groupValue: selected,
            onChanged: onSelected,
            child: Column(
              children: [
                if (none != null)
                  RadioListTile<String?>(
                    dense: true,
                    value: null,
                    title: Text(none),
                  ),
                for (final item in list ?? const <DirectoryItem>[])
                  RadioListTile<String?>(
                    dense: true,
                    value: item.id,
                    secondary: NodeKindGlyph(kind: icon, decorative: true),
                    title: Text(item.title),
                    subtitle: item.subtitle == null
                        ? null
                        : Text(
                            item.subtitle!,
                            style: text.caption.copyWith(color: colors.text2),
                          ),
                  ),
              ],
            ),
          ),
        ),
      ],
    );
  }
}

/// Fires [showRecordMove] without awaiting it.
void openRecordMove(
  BuildContext context, {
  EntityRef? document,
  List<EntityRef> location = const [],
  List<DocumentBrief> documents = const [],
}) => unawaited(
  showRecordMove(
    context,
    document: document,
    location: location,
    documents: documents,
  ),
);
