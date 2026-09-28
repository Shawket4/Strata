// Reads the platform folders of this package: the test runs with the package
// root as its working directory.
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/painting.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';

/// Checks the generated app icons and native splash screens (docs/RUNBOOK.md
/// §13): every required size exists, iOS icons carry no alpha, the Windows
/// icon holds every size, and every native launch colour equals the Flutter
/// splash background token so starting the app shows no flash.
void main() {
  final light = StrataColors.light.background;
  final dark = StrataColors.dark.background;

  group('native splash matches the Flutter splash background', () {
    test('the tokens are mist and abyss', () {
      expect(light, const Color(0xFFF1F5F7));
      expect(dark, const Color(0xFF0F1B26));
    });

    test('flutter_native_splash.yaml (Android, Android 12+, iOS)', () {
      final config = File('flutter_native_splash.yaml').readAsStringSync();
      String value(String key, {bool android12 = false}) {
        final section = android12
            ? config.substring(config.indexOf('android_12:'))
            : config.substring(0, config.indexOf('android_12:'));
        final match = RegExp(
          '^\\s*$key:\\s*"(#[0-9A-Fa-f]{6})"',
          multiLine: true,
        ).firstMatch(section);
        return match!.group(1)!;
      }

      expect(_hex(value('color')), light);
      expect(_hex(value('color_dark')), dark);
      expect(_hex(value('color', android12: true)), light);
      expect(_hex(value('color_dark', android12: true)), dark);
    });

    test('Android launch and window backgrounds', () {
      const res = 'android/app/src/main/res';
      for (final (dir, colour) in [('values', light), ('values-night', dark)]) {
        final colours = File('$res/$dir/splash_colors.xml').readAsStringSync();
        expect(
          _hex(
            RegExp('<color name="strata_background">(#[0-9A-Fa-f]{6})</color>')
                .firstMatch(colours)!
                .group(1)!,
          ),
          colour,
          reason: dir,
        );
      }
      for (final (dir, colour) in [
        ('values-v31', light),
        ('values-night-v31', dark),
      ]) {
        final styles = File('$res/$dir/styles.xml').readAsStringSync();
        expect(
          _hex(
            RegExp('windowSplashScreenBackground">(#[0-9A-Fa-f]{6})<')
                .firstMatch(styles)!
                .group(1)!,
          ),
          colour,
          reason: dir,
        );
      }
      // The window behind the Flutter UI while it starts (NormalTheme) has the
      // same colour, in every styles file.
      for (final dir in [
        'values',
        'values-night',
        'values-v31',
        'values-night-v31',
      ]) {
        final styles = File('$res/$dir/styles.xml').readAsStringSync();
        final normal = styles.substring(styles.indexOf('"NormalTheme"'));
        expect(
          normal,
          contains('<item name="android:windowBackground">$_background</item>'),
          reason: dir,
        );
      }
      // Pre-12 launch drawables: the generated 1 px backgrounds.
      expect(_pixel('$res/drawable/background.png'), light);
      expect(_pixel('$res/drawable-night/background.png'), dark);
    });

    test('iOS launch screen backgrounds', () {
      const set = 'ios/Runner/Assets.xcassets/LaunchBackground.imageset';
      expect(_pixel('$set/background.png'), light);
      expect(_pixel('$set/darkbackground.png'), dark);
    });

    test('macOS window background until the first frame', () {
      final swift = File('macos/Runner/MainFlutterWindow.swift')
          .readAsStringSync();
      final colours = RegExp(
        r'NSColor\(srgbRed: 0x(\w\w) / 255\.0, green: 0x(\w\w) / 255\.0, '
        r'blue: 0x(\w\w) / 255\.0',
      ).allMatches(swift).map((m) => _hex('#${m[1]}${m[2]}${m[3]}')).toList();
      // Dark first, then light (`bestMatch == .darkAqua ? dark : light`).
      expect(colours, [dark, light]);
      expect(swift, contains('flutterViewController.backgroundColor'));
    });
  });

  group('app icons exist at the required sizes', () {
    test('Android legacy, adaptive and monochrome icons', () {
      const res = 'android/app/src/main/res';
      const densities = {
        'mdpi': 1.0,
        'hdpi': 1.5,
        'xhdpi': 2.0,
        'xxhdpi': 3.0,
        'xxxhdpi': 4.0,
      };
      for (final MapEntry(key: density, value: scale) in densities.entries) {
        expect(
          _size('$res/mipmap-$density/ic_launcher.png'),
          (48 * scale).round(),
          reason: density,
        );
        for (final layer in ['foreground', 'monochrome']) {
          expect(
            _size('$res/drawable-$density/ic_launcher_$layer.png'),
            (108 * scale).round(),
            reason: '$layer $density',
          );
        }
        expect(
          _size('$res/drawable-$density/android12splash.png'),
          (288 * scale).round(),
          reason: 'android12splash $density',
        );
      }
      final adaptive = File('$res/mipmap-anydpi-v26/ic_launcher.xml')
          .readAsStringSync();
      expect(adaptive, contains('@color/ic_launcher_background'));
      expect(adaptive, contains('@drawable/ic_launcher_foreground'));
      expect(adaptive, contains('@drawable/ic_launcher_monochrome'));
      expect(
        File('$res/values/colors.xml').readAsStringSync(),
        contains('<color name="ic_launcher_background">#0F1B26</color>'),
      );
    });

    test('iOS app icons: every listed size, no alpha channel', () {
      const dir = 'ios/Runner/Assets.xcassets/AppIcon.appiconset';
      final images = _contents(dir);
      expect(images.length, greaterThanOrEqualTo(18));
      for (final image in images) {
        final path = '$dir/${image['filename']}';
        expect(_size(path), _pixels(image), reason: path);
        expect(_colourType(path), 2, reason: '$path must be RGB, no alpha');
      }
      expect(
        images.map((i) => i['idiom']),
        containsAll(<String>['iphone', 'ipad', 'ios-marketing']),
      );
    });

    test('macOS AppIcon.appiconset: the full icns set', () {
      const dir = 'macos/Runner/Assets.xcassets/AppIcon.appiconset';
      final images = _contents(dir);
      final pixels = <int>{};
      for (final image in images) {
        final path = '$dir/${image['filename']}';
        expect(_size(path), _pixels(image), reason: path);
        pixels.add(_pixels(image));
      }
      expect(images.map((i) => '${i['size']}@${i['scale']}').toSet(), {
        for (final s in [16, 32, 128, 256, 512]) ...{
          '${s}x$s@1x',
          '${s}x$s@2x',
        },
      });
      expect(pixels, {16, 32, 64, 128, 256, 512, 1024});
    });

    test('Windows .ico holds every size', () {
      final ico = File('windows/runner/resources/app_icon.ico')
          .readAsBytesSync();
      final data = ByteData.sublistView(ico);
      expect(data.getUint16(2, Endian.little), 1, reason: 'type: icon');
      final count = data.getUint16(4, Endian.little);
      // Each 16-byte directory entry starts with the width (0 means 256).
      final sizes = [
        for (var i = 0; i < count; i++) (ico[6 + 16 * i] + 255) % 256 + 1,
      ]..sort();
      expect(sizes, [16, 20, 24, 32, 40, 48, 64, 128, 256]);
    });

    test('Linux icon and desktop entry', () {
      expect(_size('linux/runner/resources/app_icon.png'), 256);
      final desktop = File('linux/runner/resources/app.strata.strata.desktop')
          .readAsStringSync();
      expect(desktop, contains('Name=Strata\n'));
      expect(desktop, contains('Icon=app.strata.strata\n'));
      expect(desktop, contains('StartupWMClass=app.strata.strata\n'));
      final cmake = File('linux/CMakeLists.txt').readAsStringSync();
      expect(cmake, contains('set(APPLICATION_ID "app.strata.strata")'));
      expect(cmake, contains('"runner/resources/app_icon.png"'));
    });

    test('brand source art', () {
      for (final (name, size) in [
        ('icon-ios', 1024),
        ('icon-macos', 1024),
        ('icon-legacy', 1024),
        ('adaptive-foreground', 1024),
        ('adaptive-monochrome', 1024),
        ('splash-mark', 384),
        ('splash-android12', 1152),
      ]) {
        expect(_size('assets/brand/$name.png'), size, reason: name);
      }
      expect(_colourType('assets/brand/icon-ios.png'), 2);
    });
  });

  test('the display name is "Strata" on every platform', () {
    expect(
      File('android/app/src/main/AndroidManifest.xml').readAsStringSync(),
      contains('android:label="Strata"'),
    );
    for (final plist in ['ios/Runner/Info.plist', 'macos/Runner/Info.plist']) {
      final text = File(plist).readAsStringSync();
      expect(
        text,
        contains('<key>CFBundleName</key>\n\t<string>Strata</string>'),
        reason: plist,
      );
      expect(
        text,
        contains('<key>CFBundleDisplayName</key>\n\t<string>Strata</string>'),
        reason: plist,
      );
    }
    expect(
      File('windows/runner/main.cpp').readAsStringSync(),
      contains('window.Create(L"Strata"'),
    );
    expect(
      File('linux/runner/my_application.cc').readAsStringSync(),
      contains('gtk_window_set_title(window, "Strata")'),
    );
  });
}

/// The window background resource of every Android theme.
const _background = '@color/strata_background';

Color _hex(String hex) =>
    Color(0xFF000000 | int.parse(hex.substring(1), radix: 16));

List<Map<String, dynamic>> _contents(String dir) {
  final json = jsonDecode(
    File('$dir/Contents.json').readAsStringSync(),
  ) as Map<String, dynamic>;
  return (json['images'] as List).cast<Map<String, dynamic>>();
}

/// Pixel size of an asset-catalog entry (`size` × `scale`).
int _pixels(Map<String, dynamic> image) {
  final points = double.parse((image['size'] as String).split('x').first);
  final scale = int.parse((image['scale'] as String).replaceAll('x', ''));
  return (points * scale).round();
}

/// The PNG header (IHDR) of [path]: width, height, colour type.
({int width, int height, int colourType}) _header(String path) {
  final bytes = File(path).readAsBytesSync();
  const signature = [137, 80, 78, 71, 13, 10, 26, 10];
  expect(bytes.sublist(0, 8), signature, reason: '$path is not a PNG');
  final data = ByteData.sublistView(bytes);
  return (
    width: data.getUint32(16),
    height: data.getUint32(20),
    colourType: bytes[25],
  );
}

/// The side of the square PNG at [path].
int _size(String path) {
  final header = _header(path);
  expect(header.width, header.height, reason: '$path is not square');
  return header.width;
}

int _colourType(String path) => _header(path).colourType;

/// The colour of the first pixel of an 8-bit RGB/RGBA PNG without
/// interlacing (the generated backgrounds are one colour).
Color _pixel(String path) {
  final bytes = File(path).readAsBytesSync();
  final header = _header(path);
  final data = ByteData.sublistView(bytes);
  final idat = BytesBuilder();
  var offset = 8;
  while (offset < bytes.length) {
    final length = data.getUint32(offset);
    final type = ascii.decode(bytes.sublist(offset + 4, offset + 8));
    if (type == 'IDAT') {
      idat.add(bytes.sublist(offset + 8, offset + 8 + length));
    }
    offset += 12 + length;
  }
  final raw = zlib.decode(idat.takeBytes());
  // raw[0] is the first row's filter type; every filter predicts 0 for the
  // first pixel of the first row, so its bytes are the colour itself.
  expect(raw[0], inInclusiveRange(0, 4), reason: path);
  expect([2, 6], contains(header.colourType), reason: path);
  return Color.fromARGB(0xFF, raw[1], raw[2], raw[3]);
}
