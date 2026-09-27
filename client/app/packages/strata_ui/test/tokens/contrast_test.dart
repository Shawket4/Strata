import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';

/// WCAG 2 contrast ratio of two opaque colours.
double _contrast(Color a, Color b) {
  final la = a.computeLuminance();
  final lb = b.computeLuminance();
  final hi = la > lb ? la : lb;
  final lo = la > lb ? lb : la;
  return (hi + 0.05) / (lo + 0.05);
}

/// Normal-size text (WCAG AA 1.4.3).
const double _aa = 4.5;

/// The Material 3 state-layer opacities of a filled button (hovered 8 %,
/// focused and pressed 10 %); the check also covers 12 % for dragged.
const Map<String, Set<WidgetState>> _states = {
  'rest': {},
  'hovered': {WidgetState.hovered},
  'focused': {WidgetState.focused},
  'pressed': {WidgetState.pressed},
};

/// Every foreground/background pair a filled button of [style] shows: the
/// rest colours and the foreground over the fill with each state layer.
Map<String, (Color, Color)> _buttonPairs(String name, ButtonStyle style) {
  final pairs = <String, (Color, Color)>{};
  for (final entry in _states.entries) {
    final fg = style.foregroundColor!.resolve(entry.value)!;
    final fill = style.backgroundColor!.resolve(entry.value)!;
    final overlay = style.overlayColor?.resolve(entry.value);
    final bg = overlay == null ? fill : Color.alphaBlend(overlay, fill);
    pairs['$name ${entry.key}'] = (fg, bg);
  }
  return pairs;
}

void _expectPairs(Map<String, (Color, Color)> pairs, String theme) {
  for (final MapEntry(key: name, value: (fg, bg)) in pairs.entries) {
    expect(bg.a, 1, reason: '$name in $theme: background must be opaque');
    expect(
      _contrast(fg, bg),
      greaterThanOrEqualTo(_aa),
      reason:
          '$name in $theme: ${_contrast(fg, bg).toStringAsFixed(2)}:1 '
          '(fg $fg on bg $bg)',
    );
  }
}

void main() {
  for (final (name, colors, theme) in [
    ('light', StrataColors.light, StrataTheme.light()),
    ('dark', StrataColors.dark, StrataTheme.dark()),
  ]) {
    group('$name contrast', () {
      final c = colors;
      final scheme = theme.colorScheme;

      test('filled button tokens are text-safe in every state', () {
        final filled = FilledButton.styleFrom(
          backgroundColor: c.accentFill,
          foregroundColor: c.onAccentFill,
        );
        final destructive = FilledButton.styleFrom(
          backgroundColor: scheme.error,
          foregroundColor: scheme.onError,
        );
        _expectPairs({
          ..._buttonPairs('accentFill', filled),
          ..._buttonPairs('error (destructive)', destructive),
          'onAccentFill/accentFill dragged 12%': (
            c.onAccentFill,
            Color.alphaBlend(
              c.onAccentFill.withValues(alpha: 0.12),
              c.accentFill,
            ),
          ),
          'onDanger/danger dragged 12%': (
            c.onDanger,
            Color.alphaBlend(c.onDanger.withValues(alpha: 0.12), c.danger),
          ),
        }, name);
      });

      test('the themed buttons use the text-safe tokens', () {
        final filled = theme.filledButtonTheme.style!;
        expect(filled.backgroundColor!.resolve({}), c.accentFill);
        expect(filled.foregroundColor!.resolve({}), c.onAccentFill);
        expect(theme.floatingActionButtonTheme.backgroundColor, c.accentFill);
        expect(theme.floatingActionButtonTheme.foregroundColor, c.onAccentFill);
        expect(scheme.primary, c.accentFill);
        expect(scheme.onPrimary, c.onAccentFill);
        expect(scheme.error, c.danger);
        expect(scheme.onError, c.onDanger);

        final outlined = theme.outlinedButtonTheme.style!;
        final text = theme.textButtonTheme.style!;
        final fab = theme.floatingActionButtonTheme;
        _expectPairs({
          ..._buttonPairs('themed FilledButton', filled),
          'FAB': (fab.foregroundColor!, fab.backgroundColor!),
          for (final (ground, bg) in [
            ('background', c.background),
            ('surface', c.surface),
            ('surface2', c.surface2),
          ]) ...{
            'OutlinedButton on $ground': (
              outlined.foregroundColor!.resolve({})!,
              bg,
            ),
            'TextButton on $ground': (text.foregroundColor!.resolve({})!, bg),
            'error text on $ground': (scheme.error, bg),
          },
        }, name);
      });

      test('the colour scheme pairs meet AA', () {
        _expectPairs({
          'onPrimary/primary': (scheme.onPrimary, scheme.primary),
          'onError/error': (scheme.onError, scheme.error),
          'onPrimaryContainer/primaryContainer': (
            scheme.onPrimaryContainer,
            scheme.primaryContainer,
          ),
          'onSecondaryContainer/secondaryContainer': (
            scheme.onSecondaryContainer,
            scheme.secondaryContainer,
          ),
          'onTertiaryContainer/tertiaryContainer': (
            scheme.onTertiaryContainer,
            scheme.tertiaryContainer,
          ),
          'onErrorContainer/errorContainer': (
            scheme.onErrorContainer,
            scheme.errorContainer,
          ),
          'onSurface/surface': (scheme.onSurface, scheme.surface),
          'onSurfaceVariant/surface': (scheme.onSurfaceVariant, scheme.surface),
          'onSurfaceVariant/surfaceContainer': (
            scheme.onSurfaceVariant,
            scheme.surfaceContainer,
          ),
          'onInverseSurface/inverseSurface': (
            scheme.onInverseSurface,
            scheme.inverseSurface,
          ),
          'inversePrimary/inverseSurface': (
            scheme.inversePrimary,
            scheme.inverseSurface,
          ),
          'primary/surface (text buttons, links)': (
            scheme.primary,
            scheme.surface,
          ),
        }, name);
      });

      test('chips and pills meet AA', () {
        _expectPairs({
          for (final tone in StatusTone.values)
            'StatusPill ${tone.name}': (
              tone.colorsIn(c).foreground,
              tone.colorsIn(c).background,
            ),
          'CitationChip label/infoTint': (c.infoText, c.infoTint),
          'CitationChip number/surface': (c.infoText, c.surface),
          'RelationChip label/surface': (c.text, c.surface),
          'RelationChip AI tag/infoTint': (c.infoText, c.infoTint),
          'KeyboardHintChip/surface': (c.text2, c.surface),
          'KeyboardHintChip on filled button': (c.onAccentFill, c.accentFill),
          'Tooltip': (
            theme.tooltipTheme.textStyle!.color!,
            (theme.tooltipTheme.decoration! as BoxDecoration).color!,
          ),
        }, name);
      });

      test('navigation meets AA', () {
        final bar = theme.navigationBarTheme;
        final rail = theme.navigationRailTheme;
        const selected = {WidgetState.selected};
        const idle = <WidgetState>{};
        _expectPairs({
          'NavigationBar selected label': (
            bar.labelTextStyle!.resolve(selected)!.color!,
            bar.backgroundColor!,
          ),
          'NavigationBar label': (
            bar.labelTextStyle!.resolve(idle)!.color!,
            bar.backgroundColor!,
          ),
          'NavigationBar selected icon/indicator': (
            bar.iconTheme!.resolve(selected)!.color!,
            bar.indicatorColor!,
          ),
          'NavigationRail selected label': (
            rail.selectedLabelTextStyle!.color!,
            rail.backgroundColor!,
          ),
          'NavigationRail label': (
            rail.unselectedLabelTextStyle!.color!,
            rail.backgroundColor!,
          ),
          'NavigationRail selected icon/indicator': (
            rail.selectedIconTheme!.color!,
            rail.indicatorColor!,
          ),
          // RailFooterDestination (Settings) draws on the rail background.
          'rail footer selected': (c.accentText, c.surface2),
          'rail footer': (c.text2, c.surface2),
          'rail footer selected icon/indicator': (c.accentText, c.accentTint),
          'SidebarItem selected': (c.accentText, c.accentTint),
          'SidebarItem': (c.text, c.surface2),
          'SidebarItem icon and count': (c.text2, c.surface2),
          'SidebarItem hovered': (
            c.text,
            Color.alphaBlend(c.border.withValues(alpha: 0.35), c.surface2),
          ),
          'SidebarItem hovered count': (
            c.text2,
            Color.alphaBlend(c.border.withValues(alpha: 0.35), c.surface2),
          ),
          'sidebar "New capture"': (c.onAccentFill, c.accentFill),
        }, name);
      });
    });
  }

  test('tide stays the lead colour for the mark and graphics', () {
    for (final c in [StrataColors.light, StrataColors.dark]) {
      expect(c.accent, StrataPalette.tide);
    }
    expect(StrataColors.light.accentFill, StrataPalette.tideText);
    expect(StrataColors.light.onAccentFill, StrataPalette.white);
    expect(StrataColors.dark.accentFill, StrataPalette.surf);
    expect(StrataColors.dark.onAccentFill, StrataPalette.abyss);
    expect(StrataColors.light.onDanger, StrataPalette.white);
    expect(StrataColors.dark.onDanger, StrataPalette.abyss);
    // White on tide is below AA under the hover overlay: never a text fill.
    final hovered = Color.alphaBlend(
      StrataPalette.white.withValues(alpha: 0.08),
      StrataPalette.tide,
    );
    expect(_contrast(StrataPalette.white, hovered), lessThan(_aa));
  });
}
