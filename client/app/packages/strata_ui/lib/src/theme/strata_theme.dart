import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:strata_ui/src/tokens/colors.dart';
import 'package:strata_ui/src/tokens/graph.dart';
import 'package:strata_ui/src/tokens/metrics.dart';
import 'package:strata_ui/src/tokens/typography.dart';

/// Builds Material 3 [ThemeData] from the Strata tokens.
abstract final class StrataTheme {
  /// The light theme (default).
  static ThemeData light({TargetPlatform? platform}) => _build(
    brightness: Brightness.light,
    colors: StrataColors.light,
    graph: StrataGraphColors.light,
    platform: platform,
  );

  /// The dark theme.
  static ThemeData dark({TargetPlatform? platform}) => _build(
    brightness: Brightness.dark,
    colors: StrataColors.dark,
    graph: StrataGraphColors.dark,
    platform: platform,
  );

  static ThemeData _build({
    required Brightness brightness,
    required StrataColors colors,
    required StrataGraphColors graph,
    TargetPlatform? platform,
  }) {
    final styles = StrataTextStyles.from(
      text: colors.text,
      text2: colors.text2,
    );
    final textTheme = styles.toTextTheme();
    final scheme = ColorScheme(
      brightness: brightness,
      primary: colors.accent,
      onPrimary: colors.onAccent,
      primaryContainer: colors.accentTint,
      onPrimaryContainer: colors.accentText,
      secondary: colors.text2,
      onSecondary: colors.surface,
      secondaryContainer: colors.accentTint,
      onSecondaryContainer: colors.accentText,
      tertiary: colors.success,
      onTertiary: colors.surface,
      tertiaryContainer: colors.successTint,
      onTertiaryContainer: colors.successText,
      error: colors.danger,
      onError: colors.onAccent,
      errorContainer: colors.dangerTint,
      onErrorContainer: colors.dangerText,
      surface: colors.surface,
      onSurface: colors.text,
      onSurfaceVariant: colors.text2,
      surfaceDim: colors.background,
      surfaceBright: colors.surface,
      surfaceContainerLowest: colors.surface,
      surfaceContainerLow: colors.surface,
      surfaceContainer: colors.surface2,
      surfaceContainerHigh: colors.surface2,
      surfaceContainerHighest: colors.surface2,
      outline: colors.border,
      outlineVariant: colors.border,
      shadow: StrataPalette.abyss,
      scrim: colors.scrim,
      inverseSurface: colors.text,
      onInverseSurface: colors.background,
      inversePrimary: StrataPalette.surf,
      surfaceTint: Colors.transparent,
    );
    final hairline = BorderSide(color: colors.border);
    const inputShape = RoundedRectangleBorder(
      borderRadius: StrataRadii.inputRadius,
    );

    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      platform: platform,
      colorScheme: scheme,
      scaffoldBackgroundColor: colors.background,
      canvasColor: colors.background,
      fontFamily: StrataFonts.cairo,
      package: StrataFonts.package,
      textTheme: textTheme,
      primaryTextTheme: textTheme,
      iconTheme: IconThemeData(color: colors.text2, size: 20),
      dividerTheme: DividerThemeData(
        color: colors.border,
        thickness: StrataElevation.hairline,
        space: StrataElevation.hairline,
      ),
      appBarTheme: AppBarTheme(
        backgroundColor: colors.background,
        foregroundColor: colors.text,
        elevation: 0,
        scrolledUnderElevation: 0,
        centerTitle: false,
        titleTextStyle: styles.titleSmall,
        systemOverlayStyle: brightness == Brightness.light
            ? SystemUiOverlayStyle.dark
            : SystemUiOverlayStyle.light,
      ),
      navigationBarTheme: NavigationBarThemeData(
        backgroundColor: colors.surface,
        indicatorColor: colors.accentTint,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
        height: 72,
        labelTextStyle: WidgetStateProperty.resolveWith(
          (states) => styles.caption
              .withWeight(FontWeight.w600)
              .copyWith(
                color: states.contains(WidgetState.selected)
                    ? colors.accentText
                    : colors.text2,
              ),
        ),
        iconTheme: WidgetStateProperty.resolveWith(
          (states) => IconThemeData(
            size: 22,
            color: states.contains(WidgetState.selected)
                ? colors.accentText
                : colors.text2,
          ),
        ),
      ),
      navigationRailTheme: NavigationRailThemeData(
        backgroundColor: colors.surface2,
        indicatorColor: colors.accentTint,
        minWidth: StrataLayout.railWidth,
        selectedIconTheme: IconThemeData(color: colors.accentText, size: 22),
        unselectedIconTheme: IconThemeData(color: colors.text2, size: 22),
        selectedLabelTextStyle: styles.caption
            .withWeight(FontWeight.w600)
            .copyWith(color: colors.accentText),
        unselectedLabelTextStyle: styles.caption.copyWith(color: colors.text2),
        labelType: NavigationRailLabelType.all,
      ),
      cardTheme: CardThemeData(
        color: colors.surface,
        elevation: 0,
        margin: EdgeInsets.zero,
        shape: RoundedRectangleBorder(
          borderRadius: StrataRadii.cardRadius,
          side: hairline,
        ),
      ),
      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: colors.surface,
        modalBackgroundColor: colors.surface,
        surfaceTintColor: Colors.transparent,
        showDragHandle: true,
        dragHandleColor: colors.border,
        shape: const RoundedRectangleBorder(
          borderRadius: StrataRadii.sheetTopRadius,
        ),
        modalBarrierColor: colors.scrim,
      ),
      dialogTheme: DialogThemeData(
        backgroundColor: colors.surface,
        surfaceTintColor: Colors.transparent,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.all(Radius.circular(StrataRadii.sheet)),
        ),
      ),
      inputDecorationTheme: InputDecorationThemeData(
        filled: true,
        fillColor: colors.surface2,
        hintStyle: styles.body.copyWith(color: colors.text2),
        border: OutlineInputBorder(
          borderRadius: StrataRadii.inputRadius,
          borderSide: hairline,
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: StrataRadii.inputRadius,
          borderSide: hairline,
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: StrataRadii.inputRadius,
          borderSide: BorderSide(color: colors.accent, width: 2),
        ),
      ),
      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          backgroundColor: colors.accent,
          foregroundColor: colors.onAccent,
          textStyle: styles.label,
          shape: inputShape,
        ),
      ),
      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          foregroundColor: colors.text,
          textStyle: styles.label,
          side: hairline,
          shape: inputShape,
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: colors.accentText,
          textStyle: styles.label,
          shape: inputShape,
        ),
      ),
      floatingActionButtonTheme: FloatingActionButtonThemeData(
        backgroundColor: colors.accent,
        foregroundColor: colors.onAccent,
        elevation: 0,
        focusElevation: 0,
        hoverElevation: 0,
        highlightElevation: 0,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.all(Radius.circular(StrataRadii.card)),
        ),
      ),
      tooltipTheme: TooltipThemeData(
        textStyle: styles.caption.copyWith(color: colors.background),
        decoration: BoxDecoration(
          color: colors.text,
          borderRadius: StrataRadii.inputRadius,
        ),
      ),
      extensions: <ThemeExtension<dynamic>>[
        colors,
        graph,
        styles,
        StrataMotion.standardMotion,
      ],
    );
  }
}

/// Typed access to the Strata theme extensions.
extension StrataThemeContext on BuildContext {
  /// Colour tokens of the ambient theme.
  StrataColors get strataColors =>
      Theme.of(this).extension<StrataColors>() ?? StrataColors.light;

  /// Graph (relation and node) colours of the ambient theme.
  StrataGraphColors get strataGraphColors =>
      Theme.of(this).extension<StrataGraphColors>() ?? StrataGraphColors.light;

  /// Named text styles of the ambient theme.
  StrataTextStyles get strataText =>
      Theme.of(this).extension<StrataTextStyles>() ??
      StrataTextStyles.from(
        text: StrataColors.light.text,
        text2: StrataColors.light.text2,
      );

  /// Motion tokens of the ambient theme; reduced when the platform asks for
  /// fewer animations.
  StrataMotion get strataMotion {
    final disabled = MediaQuery.maybeDisableAnimationsOf(this) ?? false;
    if (disabled) return StrataMotion.reduced;
    return Theme.of(this).extension<StrataMotion>() ??
        StrataMotion.standardMotion;
  }
}

/// Registers the OFL licences of the bundled fonts with the
/// [LicenseRegistry], so they appear on the licences page.
void registerStrataFontLicenses({AssetBundle? bundle}) {
  LicenseRegistry.addLicense(() async* {
    final assets = bundle ?? rootBundle;
    for (final path in StrataFonts.licenseAssets) {
      final name = path.split('/').last.replaceAll('-OFL.txt', '');
      final text = await assets.loadString(path);
      yield LicenseEntryWithLineBreaks([name], text);
    }
  });
}
