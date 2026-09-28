import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_settings/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart';

/// Asks for a time zone from the core's list (`timezones`): names and
/// regions in the UI language, current offsets, the account's zone marked;
/// the search is matched by the core. Returns the chosen IANA ID, or `null`
/// when cancelled.
Future<String?> showTimeZonePicker(BuildContext context) => showDialog<String>(
  context: context,
  builder: (_) => const TimeZonePickerDialog(),
);

/// The searchable time-zone list of [showTimeZonePicker].
class TimeZonePickerDialog extends HookConsumerWidget {
  /// Creates the dialog.
  const new({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final query = useState('');
    final zones = ref.watch(timeZonesProvider(query.value)).value;
    return AlertDialog(
      title: Text(l10n.editTimezoneTitle),
      content: SizedBox(
        width: 440,
        height: 420,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              l10n.timezoneHelp,
              style: text.caption.copyWith(color: colors.text2),
            ),
            const SizedBox(height: StrataSpacing.s2),
            TextField(
              autofocus: true,
              onChanged: (value) => query.value = value,
              decoration: InputDecoration(
                isDense: true,
                hintText: l10n.timezoneSearch,
                prefixIcon: const Icon(Icons.search, size: 20),
              ),
            ),
            const SizedBox(height: StrataSpacing.s2),
            Expanded(
              child: zones == null
                  ? const Center(child: CircularProgressIndicator())
                  : zones.isEmpty
                  ? Center(
                      child: Text(
                        l10n.timezoneNoMatch(query: query.value),
                        textAlign: TextAlign.center,
                        style: text.bodySmall.copyWith(color: colors.text2),
                      ),
                    )
                  : ListView.builder(
                      itemCount: zones.length,
                      itemBuilder: (context, index) {
                        final zone = zones[index];
                        return ListTile(
                          dense: true,
                          selected: zone.isCurrent,
                          title: Text(
                            zone.name,
                            textDirection: textDirectionOf(zone.nameDir),
                            textAlign: TextAlign.start,
                          ),
                          subtitle: Wrap(
                            spacing: StrataSpacing.s2,
                            children: [
                              if (zone.region.isNotEmpty)
                                Text(
                                  zone.region,
                                  style: text.caption.copyWith(
                                    color: colors.text2,
                                  ),
                                ),
                              Text(
                                zone.id,
                                textDirection: TextDirection.ltr,
                                style: text.monoSmall.copyWith(
                                  color: colors.text2,
                                ),
                              ),
                            ],
                          ),
                          trailing: Wrap(
                            spacing: StrataSpacing.s2,
                            crossAxisAlignment: WrapCrossAlignment.center,
                            children: [
                              if (zone.isCurrent)
                                StatusPill(
                                  label: l10n.timezoneCurrent,
                                  tone: StatusTone.info,
                                ),
                              Text(
                                zone.offsetLabel,
                                textDirection: TextDirection.ltr,
                                style: text.monoSmall,
                              ),
                            ],
                          ),
                          onTap: () => Navigator.of(context).pop(zone.id),
                        );
                      },
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
