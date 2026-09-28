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

/// The "Record a move" form: what happened, where it went (the core's
/// place list, nested under their parents, with the current place marked),
/// who has it after, the third party (sent to), the date and an optional
/// note. "Record move" writes the custody event through the core
/// (`record_custody`); the core says which field an event needs, and a date
/// left at "Today" is today in the account's time zone (computed there).
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
    final thirdParty = useState<String?>(null);
    final chosenDocument = useState<String?>(document?.id);
    final date = useState<DateTime?>(null);
    final note = useTextEditingController();
    final personQuery = useState('');
    final companyQuery = useState('');
    final busy = useState(false);
    final places = ref.watch(placeOptionsProvider(chosenDocument.value)).value;
    final people = ref
        .watch(directoryProvider(DirectoryTab.people, personQuery.value))
        .value
        ?.items;
    final companies = event.value == MoveEvent.sentTo
        ? ref
              .watch(
                directoryProvider(DirectoryTab.companies, companyQuery.value),
              )
              .value
              ?.items
        : null;
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
          Semantics(
            container: true,
            header: true,
            child: Text(name, style: label),
          ),
          const SizedBox(height: StrataSpacing.s1),
          child,
        ],
      ),
    );

    Future<void> submit() async {
      final messenger = ScaffoldMessenger.maybeOf(context);
      final documentId = chosenDocument.value;
      if (documentId == null) {
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(l10n.chooseDocument)));
        return;
      }
      busy.value = true;
      // A picked calendar day travels as that day; none means today in the
      // account's zone, which the core works out.
      final day = date.value;
      try {
        await ref
            .read(coreApiProvider)
            .recordCustody(
              documentId: documentId,
              draft: CustodyDraft(
                kind: event.value.wire,
                placeId: place.value,
                personId: holder.value,
                counterpartyId: event.value == MoveEvent.sentTo
                    ? thirdParty.value
                    : null,
                date: day == null
                    ? null
                    : DateTime.utc(day.year, day.month, day.day),
                note: note.text,
              ),
            );
        if (!context.mounted) return;
        await Navigator.of(context).maybePop();
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(l10n.moveRecorded)));
      } on CoreFailure catch (error) {
        final message = switch ((error.code, error.field)) {
          ('invalid_input', 'place_id') => l10n.choosePlace,
          ('invalid_input', 'person_id') => l10n.choosePerson,
          ('invalid_input', 'counterparty_id') => l10n.chooseThirdParty,
          _ => l10n.moveFailed(code: error.code),
        };
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(message)));
      } on Object catch (error, stack) {
        // Recording a move is local (outbox): a generic message.
        reportUntypedFailure(error, stack, action: 'recording a move');
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(
            SnackBar(content: Text(l10n.moveFailed(code: 'internal'))),
          );
      } finally {
        if (context.mounted) busy.value = false;
      }
    }

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
                  _PlacePicker(
                    options: places,
                    selected: place.value,
                    onSelected: (value) => place.value = value,
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
                if (event.value == MoveEvent.sentTo)
                  field(
                    l10n.thirdParty,
                    _Picker(
                      searchLabel: l10n.searchCompanies,
                      onQuery: (value) => companyQuery.value = value,
                      selected: thirdParty.value,
                      onSelected: (value) => thirdParty.value = value,
                      items: companies,
                      icon: NodeKind.company,
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
                  TextField(
                    controller: note,
                    minLines: 1,
                    maxLines: 3,
                    decoration: InputDecoration(
                      isDense: true,
                      hintText: l10n.noteHint,
                    ),
                  ),
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
                      onPressed: busy.value ? null : submit,
                      style: FilledButton.styleFrom(
                        minimumSize: const Size(48, 48),
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

/// The core's places (`place_options`), each indented by its depth with its
/// breadcrumb; the document's current place is marked.
class _PlacePicker extends StatelessWidget {
  const new({
    required this.options,
    required this.selected,
    required this.onSelected,
  });

  final List<PlaceOption>? options;
  final String? selected;
  final ValueChanged<String?> onSelected;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return DecoratedBox(
      decoration: BoxDecoration(
        border: Border.all(color: colors.border),
        borderRadius: StrataRadii.inputRadius,
      ),
      child: RadioGroup<String?>(
        groupValue: selected,
        onChanged: onSelected,
        child: Column(
          children: [
            for (final option in options ?? const <PlaceOption>[])
              Padding(
                padding: EdgeInsetsDirectional.only(
                  start: StrataSpacing.s4 * option.depth,
                ),
                child: RadioListTile<String?>(
                  dense: true,
                  value: option.id,
                  secondary: option.isCurrent
                      ? StatusPill(
                          label: l10n.currentPlace,
                          tone: StatusTone.info,
                        )
                      : null,
                  title: Text(option.title),
                  subtitle: option.breadcrumb.isEmpty
                      ? null
                      : Text(
                          option.breadcrumb.last.title,
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                ),
              ),
          ],
        ),
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
