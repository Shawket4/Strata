import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';

double _contrast(Color a, Color b) {
  final la = a.computeLuminance();
  final lb = b.computeLuminance();
  final hi = la > lb ? la : lb;
  final lo = la > lb ? lb : la;
  return (hi + 0.05) / (lo + 0.05);
}

void main() {
  group('StrataColors', () {
    test('light tokens match the design spec', () {
      const c = StrataColors.light;
      expect(c.background, const Color(0xFFF1F5F7));
      expect(c.surface, const Color(0xFFFFFFFF));
      expect(c.surface2, const Color(0xFFE8EFF2));
      expect(c.border, const Color(0xFFCBD8DE));
      expect(c.text, const Color(0xFF0F1B26));
      expect(c.text2, const Color(0xFF52616B));
      expect(c.text3, const Color(0xFF7C8C96));
      expect(c.accent, const Color(0xFF2477B3));
      expect(c.accentText, const Color(0xFF1D5C8C));
      expect(c.accentTint, const Color(0xFFDCEAF4));
      expect(c.sand, const Color(0xFFD8B47E));
      expect(c.danger, const Color(0xFFB3412E));
      expect(c.warning, const Color(0xFF9A6A12));
      expect(c.success, const Color(0xFF2F7A55));
      expect(c.successTint, const Color(0xFFEAF4EE));
      expect(c.warningTint, const Color(0xFFF5ECDA));
      expect(c.warningText, const Color(0xFF6E4B0C));
      expect(c.dangerTint, const Color(0xFFF6E3DF));
      expect(c.dangerText, const Color(0xFF9A3624));
      expect(c.infoTint, c.accentTint);
    });

    test('dark tokens match the design spec', () {
      const c = StrataColors.dark;
      expect(c.background, const Color(0xFF0F1B26));
      expect(c.surface, const Color(0xFF15283A));
      expect(c.surface2, const Color(0xFF1B3044));
      expect(c.border, const Color(0xFF23384A));
      expect(c.text, const Color(0xFFF1F5F7));
      expect(c.text2, const Color(0xFF93A3AD));
      expect(c.accent, const Color(0xFF2477B3));
      expect(c.accentText, const Color(0xFF6CB4DD));
      expect(c.accentTint, const Color(0xFF1B3A55));
    });

    test('palette names the coastal colours', () {
      expect(StrataPalette.tide, const Color(0xFF2477B3));
      expect(StrataPalette.abyss, const Color(0xFF0F1B26));
      expect(StrataPalette.mist, const Color(0xFFF1F5F7));
      expect(StrataPalette.harbour, const Color(0xFF7C8C96));
      expect(StrataPalette.surf, const Color(0xFF6CB4DD));
      expect(StrataPalette.sand, const Color(0xFFD8B47E));
    });

    for (final (name, c) in [
      ('light', StrataColors.light),
      ('dark', StrataColors.dark),
    ]) {
      test('$name text pairs meet WCAG AA (4.5:1)', () {
        final pairs = <String, (Color, Color)>{
          'text/background': (c.text, c.background),
          'text/surface': (c.text, c.surface),
          'text/surface2': (c.text, c.surface2),
          'text2/background': (c.text2, c.background),
          'text2/surface': (c.text2, c.surface),
          'text2/surface2': (c.text2, c.surface2),
          'accentText/surface': (c.accentText, c.surface),
          'accentText/accentTint': (c.accentText, c.accentTint),
          'onAccent/accent': (c.onAccent, c.accent),
          'successText/successTint': (c.successText, c.successTint),
          'warningText/warningTint': (c.warningText, c.warningTint),
          'dangerText/dangerTint': (c.dangerText, c.dangerTint),
          'infoText/infoTint': (c.infoText, c.infoTint),
        };
        for (final entry in pairs.entries) {
          final (fg, bg) = entry.value;
          expect(
            _contrast(fg, bg),
            greaterThanOrEqualTo(4.5),
            reason: '${entry.key} in $name',
          );
        }
      });
    }

    test('lerp interpolates and copyWith replaces', () {
      final mid = StrataColors.light.lerp(StrataColors.dark, 0.5);
      expect(
        mid.background,
        Color.lerp(
          StrataColors.light.background,
          StrataColors.dark.background,
          0.5,
        ),
      );
      expect(StrataColors.light.lerp(null, 0.5), same(StrataColors.light));
      final copy = StrataColors.light.copyWith(accent: const Color(0xFF000000));
      expect(copy.accent, const Color(0xFF000000));
      expect(copy.text, StrataColors.light.text);
    });
  });

  group('graph tokens', () {
    test('relation colours match the spec', () {
      const g = StrataGraphColors.light;
      expect(g.relation(RelationType.related), const Color(0xFF52616B));
      expect(g.relation(RelationType.partOf), const Color(0xFF0F1B26));
      expect(g.relation(RelationType.supports), const Color(0xFF2F7A55));
      expect(g.relation(RelationType.contradicts), const Color(0xFFB3412E));
      expect(g.relation(RelationType.followsUp), const Color(0xFF1D5C8C));
      expect(g.relation(RelationType.duplicates), const Color(0xFF7C8C96));
      expect(g.relation(RelationType.similarity), const Color(0xFF7C8C96));
      expect(g.relation(RelationType.bodyLink), const Color(0xFF9AAAB3));
      expect(
        g.relation(RelationType.mention, mentionOf: NodeKind.company),
        const Color(0xFF9A6A12),
      );
      expect(g.relation(RelationType.mention), const Color(0xFF2F7A55));
    });

    test('relation line styles match the spec', () {
      final related = RelationLineStyle.of(RelationType.related);
      expect(related.strokeWidth, 1.5);
      expect(related.dashPattern, isNull);
      expect(related.arrow, isFalse);

      final partOf = RelationLineStyle.of(RelationType.partOf);
      expect((partOf.strokeWidth, partOf.arrow), (2, true));

      final contradicts = RelationLineStyle.of(RelationType.contradicts);
      expect(contradicts.dashPattern, [6, 4]);
      expect(contradicts.arrow, isTrue);
      expect(contradicts.midCross, isTrue);

      final followsUp = RelationLineStyle.of(RelationType.followsUp);
      expect(followsUp.dashPattern, [2, 3]);
      expect(followsUp.arrow, isTrue);

      expect(RelationLineStyle.of(RelationType.duplicates).doubleLine, isTrue);

      final similarity = RelationLineStyle.of(RelationType.similarity);
      expect(similarity.dashPattern, [1, 4]);
      expect(similarity.opacity, 0.4);
      expect(similarity.strokeWidth, 1);

      expect(RelationLineStyle.of(RelationType.bodyLink).strokeWidth, 1);
    });

    test('no two relation types share both colour and line pattern', () {
      const g = StrataGraphColors.light;
      final signatures = <String>{};
      for (final type in RelationType.values) {
        if (type == RelationType.mention) continue;
        final s = RelationLineStyle.of(type);
        signatures.add(
          '${g.relation(type)}|${s.strokeWidth}|${s.dashPattern}|${s.arrow}|'
          '${s.doubleLine}|${s.midCross}|${s.opacity}',
        );
      }
      expect(signatures, hasLength(RelationType.values.length - 1));
    });

    test('node kinds have spec shapes and colours', () {
      expect(NodeKind.values.map(nodeShapeOf).toList(), [
        NodeShape.circle,
        NodeShape.diamond,
        NodeShape.ringedCircle,
        NodeShape.roundedSquare,
        NodeShape.foldedPage,
        NodeShape.pin,
      ]);
      const l = StrataGraphColors.light;
      expect(l.node(NodeKind.note).fill, const Color(0xFF0F1B26));
      expect(l.node(NodeKind.concept).fill, const Color(0xFFDCEAF4));
      expect(l.node(NodeKind.concept).stroke, const Color(0xFF2477B3));
      expect(l.node(NodeKind.person).fill, const Color(0xFF2F7A55));
      expect(l.node(NodeKind.company).fill, const Color(0xFF9A6A12));
      expect(l.node(NodeKind.document).fill, const Color(0xFF3F4C55));
      expect(l.node(NodeKind.place).fill, isNull);
      expect(l.node(NodeKind.place).stroke, const Color(0xFF1D5C8C));
      const d = StrataGraphColors.dark;
      expect(d.node(NodeKind.note).fill, const Color(0xFFF1F5F7));
      expect(d.node(NodeKind.person).fill, const Color(0xFF6CC495));
      expect(d.node(NodeKind.company).fill, const Color(0xFFC99A3E));
      expect(d.node(NodeKind.document).fill, const Color(0xFFB9C8D0));
      expect(d.node(NodeKind.place).stroke, const Color(0xFF6CB4DD));
      expect(l.clusterFill, const Color(0x122477B3));
    });
  });

  group('metrics', () {
    test('spacing is on the 4 px grid', () {
      for (final value in const [
        StrataSpacing.s1,
        StrataSpacing.s2,
        StrataSpacing.s3,
        StrataSpacing.s4,
        StrataSpacing.s5,
        StrataSpacing.s6,
        StrataSpacing.s8,
        StrataSpacing.s10,
        StrataSpacing.s12,
      ]) {
        expect(value % 4, 0);
      }
    });

    test('radii, elevation and layout metrics', () {
      expect(
        (StrataRadii.input, StrataRadii.card, StrataRadii.sheet),
        (8.0, 12.0, 16.0),
      );
      expect(StrataElevation.popover.single.offset, const Offset(0, 8));
      expect(StrataElevation.popover.single.blurRadius, 24);
      expect(StrataElevation.popover.single.color, const Color(0x1F0F1B26));
      expect(StrataLayout.railWidth, 80);
      expect(StrataLayout.sidebarWidth, 248);
      expect(StrataLayout.listPaneWidth, 320);
      expect(StrataLayout.contextPanelWidth, 340);
      expect(StrataLayout.maxCompactDestinations, 5);
    });

    test('touch platforms need 48 px targets, pointer platforms 32', () {
      expect(TargetPlatform.values.where(StrataLayout.isTouch).toSet(), {
        TargetPlatform.android,
        TargetPlatform.iOS,
        TargetPlatform.fuchsia,
      });
    });

    test('reduced motion zeroes every duration', () {
      const r = StrataMotion.reduced;
      expect([r.short, r.medium, r.long], everyElement(Duration.zero));
      const s = StrataMotion.standardMotion;
      expect(
        [s.short, s.medium, s.long].map((d) => d.inMilliseconds).toList(),
        [120, 200, 320],
      );
    });
  });

  group('SizeClass', () {
    test('breakpoints at 600 and 1200', () {
      expect(SizeClass.fromWidth(0), SizeClass.compact);
      expect(SizeClass.fromWidth(390), SizeClass.compact);
      expect(SizeClass.fromWidth(599.9), SizeClass.compact);
      expect(SizeClass.fromWidth(600), SizeClass.medium);
      expect(SizeClass.fromWidth(1024), SizeClass.medium);
      expect(SizeClass.fromWidth(1199.9), SizeClass.medium);
      expect(SizeClass.fromWidth(1200), SizeClass.expanded);
      expect(SizeClass.fromWidth(1920), SizeClass.expanded);
    });
  });

  group('typography', () {
    test('scale and families follow the spec', () {
      final t = StrataTextStyles.from(
        text: StrataColors.light.text,
        text2: StrataColors.light.text2,
      );
      expect(
        [
          t.caption,
          t.bodySmall,
          t.body,
          t.titleSmall,
          t.title,
          t.display,
        ].map((s) => s.fontSize).toList(),
        [12, 14, 16, 18, 22, 28],
      );
      expect(t.body.height, 1.5);
      expect(t.body.fontFamily, 'packages/strata_ui/Cairo');
      expect(t.mono.fontFamily, 'packages/strata_ui/IBMPlexMono');
      expect(t.wordmark.fontFamily, 'packages/strata_ui/Quicksand');
      expect(t.wordmark.fontWeight, FontWeight.w700);
      expect(t.wordmark.fontVariations, [const FontVariation.weight(700)]);
      expect(t.caption.color, StrataColors.light.text2);
    });

    test('withWeight keeps the variable-font axis in sync', () {
      final s = strataTextStyle(family: StrataFonts.cairo, size: 14);
      final bold = s.withWeight(FontWeight.w600);
      expect(bold.fontWeight, FontWeight.w600);
      expect(bold.fontVariations, [const FontVariation.weight(600)]);
    });
  });

  group('StrataTheme', () {
    for (final (name, theme, colors) in [
      ('light', StrataTheme.light(), StrataColors.light),
      ('dark', StrataTheme.dark(), StrataColors.dark),
    ]) {
      test('$name builds Material 3 from the tokens', () {
        expect(theme.useMaterial3, isTrue);
        expect(
          theme.brightness,
          name == 'light' ? Brightness.light : Brightness.dark,
        );
        expect(theme.extension<StrataColors>(), colors);
        expect(theme.extension<StrataGraphColors>(), isNotNull);
        expect(theme.extension<StrataTextStyles>(), isNotNull);
        expect(theme.extension<StrataMotion>(), StrataMotion.standardMotion);
        expect(theme.colorScheme.primary, colors.accent);
        expect(theme.colorScheme.surface, colors.surface);
        expect(theme.colorScheme.onSurface, colors.text);
        expect(theme.colorScheme.error, colors.danger);
        expect(theme.scaffoldBackgroundColor, colors.background);
        expect(theme.navigationRailTheme.minWidth, StrataLayout.railWidth);
        expect(theme.navigationBarTheme.indicatorColor, colors.accentTint);
        expect(theme.dividerTheme.color, colors.border);
        expect(
          theme.textTheme.bodyLarge?.fontFamily,
          'packages/strata_ui/Cairo',
        );
      });
    }

    testWidgets('context accessors fall back to light tokens', (tester) async {
      late BuildContext ctx;
      await tester.pumpWidget(
        Builder(
          builder: (context) {
            ctx = context;
            return const SizedBox();
          },
        ),
      );
      expect(ctx.strataColors, StrataColors.light);
      expect(ctx.strataGraphColors, StrataGraphColors.light);
      expect(ctx.strataMotion, StrataMotion.standardMotion);
    });

    testWidgets('reduced motion when the platform disables animations', (
      tester,
    ) async {
      late BuildContext ctx;
      await tester.pumpWidget(
        MediaQuery(
          data: const MediaQueryData(disableAnimations: true),
          child: Builder(
            builder: (context) {
              ctx = context;
              return const SizedBox();
            },
          ),
        ),
      );
      expect(ctx.strataMotion, StrataMotion.reduced);
    });

    testWidgets('font licences are registered with the license registry', (
      tester,
    ) async {
      registerStrataFontLicenses();
      final licenses = await tester.runAsync(
        () => LicenseRegistry.licenses.toList(),
      );
      final packages = licenses!.expand((l) => l.packages).toSet();
      expect(packages, containsAll(['Cairo', 'IBMPlexMono', 'Quicksand']));
      final cairo = licenses.firstWhere((l) => l.packages.contains('Cairo'));
      expect(
        cairo.paragraphs.map((p) => p.text).join('\n'),
        contains('SIL Open Font License, Version 1.1'),
      );
    });
  });
}
