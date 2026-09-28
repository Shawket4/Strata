import 'package:flutter/material.dart';
import 'package:strata_directory/src/common/l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// What the property dialog returns: the key and every value as typed (the
/// core trims them, drops blanks and repeats, and writes one value as a
/// scalar, several as a list: `set_property_values`).
typedef PropertyEdit = ({String key, List<String> values});

/// Asks for a property's key and values ("+ Add property" / "Edit phone"):
/// one field per value, "Add value" for another (several phone numbers).
/// Returns `null` when cancelled.
Future<PropertyEdit?> showPropertyDialog(
  BuildContext context, {
  required String title,
  String key = '',
  List<String> values = const [],
}) => showDialog<PropertyEdit>(
  context: context,
  builder: (_) => DirectoryLocalizationScope(
    child: PropertyDialog(title: title, initialKey: key, initialValues: values),
  ),
);

/// The dialog of [showPropertyDialog].
class PropertyDialog extends StatefulWidget {
  /// Creates the dialog.
  const new({
    required this.title,
    super.key,
    this.initialKey = '',
    this.initialValues = const [],
  });

  /// Title.
  final String title;

  /// The key being edited (empty for a new property).
  final String initialKey;

  /// Its current values.
  final List<String> initialValues;

  @override
  State<PropertyDialog> createState() => _PropertyDialogState();
}

class _PropertyDialogState extends State<PropertyDialog> {
  late final TextEditingController _key = TextEditingController(
    text: widget.initialKey,
  );
  late final List<TextEditingController> _values = [
    for (final value in widget.initialValues)
      TextEditingController(text: value),
    if (widget.initialValues.isEmpty) TextEditingController(),
  ];

  @override
  void dispose() {
    _key.dispose();
    for (final value in _values) {
      value.dispose();
    }
    super.dispose();
  }

  void _remove(int index) {
    setState(() => _values.removeAt(index).dispose());
  }

  @override
  Widget build(BuildContext context) {
    final l10n = context.dirL10n;
    return AlertDialog(
      title: Text(widget.title),
      scrollable: true,
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
            child: TextField(
              controller: _key,
              autofocus: widget.initialKey.isEmpty,
              decoration: InputDecoration(labelText: l10n.propertyKey),
            ),
          ),
          for (var i = 0; i < _values.length; i++)
            Padding(
              padding: const EdgeInsets.only(bottom: StrataSpacing.s2),
              child: TextField(
                controller: _values[i],
                autofocus: widget.initialKey.isNotEmpty && i == 0,
                decoration: InputDecoration(
                  labelText: l10n.propertyValue,
                  suffixIcon: _values.length < 2
                      ? null
                      : IconButton(
                          tooltip: l10n.removeValue,
                          icon: const Icon(Icons.close, size: 18),
                          onPressed: () => _remove(i),
                        ),
                ),
              ),
            ),
          Align(
            alignment: AlignmentDirectional.centerStart,
            child: TextButton.icon(
              onPressed: () =>
                  setState(() => _values.add(TextEditingController())),
              icon: const Icon(Icons.add, size: 18),
              label: Text(l10n.addValue),
            ),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop((
            key: _key.text,
            values: [for (final value in _values) value.text],
          )),
          child: Text(l10n.save),
        ),
      ],
    );
  }
}
