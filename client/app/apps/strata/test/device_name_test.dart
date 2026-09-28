import 'package:device_info_plus/device_info_plus.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata/strata.dart';
import 'package:strata_accounts/strata_accounts.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';

import 'helpers/boot.dart';
import 'helpers/fakes.dart';

/// A Samsung Galaxy S24 without a device-name setting (what Android reports
/// through `device_info_plus`; the host name is Android's "localhost").
AndroidDeviceInfo _samsung({String name = ''}) =>
    AndroidDeviceInfo.setMockInitialValues(
      version: AndroidBuildVersion.setMockInitialValues(
        codename: 'REL',
        incremental: 'S921BXXU1AWM9',
        previewSdkInt: 0,
        release: '14',
        sdkInt: 34,
      ),
      board: 's5e9945',
      bootloader: 'S921BXXU1AWM9',
      brand: 'samsung',
      device: 'e1s',
      display: 'UP1A.231005.007.S921BXXU1AWM9',
      fingerprint: 'samsung/e1sxeea/e1s:14/UP1A.231005.007/S921B:user/release',
      hardware: 's5e9945',
      host: 'build-host',
      id: 'UP1A.231005.007',
      manufacturer: 'samsung',
      model: 'SM-S921B',
      product: 'e1sxeea',
      name: name,
      supported32BitAbis: const [],
      supported64BitAbis: const ['arm64-v8a'],
      supportedAbis: const ['arm64-v8a'],
      tags: 'release-keys',
      type: 'user',
      isPhysicalDevice: true,
      freeDiskSize: 1,
      totalDiskSize: 2,
      systemFeatures: const [],
      isLowRamDevice: false,
      physicalRamSize: 8192,
      availableRamSize: 4096,
    );

const DeviceFacts _samsungFacts = DeviceFacts(
  deviceName: '',
  manufacturer: 'samsung',
  model: 'SM-S921B',
  modelName: '',
  hostName: 'localhost',
);

void main() {
  group('device facts (device_info_plus, passed unchanged)', () {
    test('Android: device name, manufacturer and model', () {
      expect(deviceFactsOf(_samsung(), hostName: 'localhost'), _samsungFacts);
      expect(
        deviceFactsOf(_samsung(name: 'Galaxy S24'), hostName: 'localhost'),
        const DeviceFacts(
          deviceName: 'Galaxy S24',
          manufacturer: 'samsung',
          model: 'SM-S921B',
          modelName: '',
          hostName: 'localhost',
        ),
      );
    });

    test('iOS: device name, model and commercial model name', () {
      final info = IosDeviceInfo.setMockInitialValues(
        name: 'iPhone',
        systemName: 'iOS',
        systemVersion: '18.0',
        model: 'iPhone',
        modelName: 'iPhone 16 Pro',
        localizedModel: 'iPhone',
        freeDiskSize: 1,
        totalDiskSize: 2,
        isPhysicalDevice: true,
        isiOSAppOnMac: false,
        isiOSAppOnVision: false,
        physicalRamSize: 8192,
        availableRamSize: 4096,
        utsname: IosUtsname.setMockInitialValues(
          sysname: 'Darwin',
          nodename: 'localhost',
          release: '24.0.0',
          version: 'Darwin Kernel',
          machine: 'iPhone17,1',
        ),
      );
      expect(
        deviceFactsOf(info, hostName: 'localhost'),
        const DeviceFacts(
          deviceName: 'iPhone',
          manufacturer: '',
          model: 'iPhone',
          modelName: 'iPhone 16 Pro',
          hostName: 'localhost',
        ),
      );
    });

    test('macOS: the computer name and model', () {
      final info = MacOsDeviceInfo.setMockInitialValues(
        computerName: 'Shawket’s MacBook Pro',
        hostName: 'Darwin',
        arch: 'arm64',
        model: 'Mac16,2',
        modelName: 'MacBook Pro',
        kernelVersion: 'Darwin Kernel',
        osRelease: '15.0',
        majorVersion: 15,
        minorVersion: 0,
        patchVersion: 0,
        activeCPUs: 10,
        memorySize: 16,
        cpuFrequency: 0,
        systemGUID: 'guid',
      );
      expect(
        deviceFactsOf(info, hostName: 'shawket-mbp.local'),
        const DeviceFacts(
          deviceName: 'Shawket’s MacBook Pro',
          manufacturer: '',
          model: 'Mac16,2',
          modelName: 'MacBook Pro',
          hostName: 'shawket-mbp.local',
        ),
      );
    });

    test('Linux (and an unknown platform): the host name only', () {
      final info = LinuxDeviceInfo(
        name: 'Ubuntu',
        id: 'ubuntu',
        prettyName: 'Ubuntu 24.04 LTS',
        machineId: 'machine',
      );
      const hostOnly = DeviceFacts(
        deviceName: '',
        manufacturer: '',
        model: '',
        modelName: '',
        hostName: 'shawket-laptop',
      );
      expect(deviceFactsOf(info, hostName: 'shawket-laptop'), hostOnly);
      expect(deviceFactsOf(null, hostName: 'shawket-laptop'), hostOnly);
    });

    test('readDeviceFacts asks the plugin once and falls back to the host '
        'name when it fails', () async {
      var asked = 0;
      expect(
        await readDeviceFacts(
          deviceInfo: () async {
            asked++;
            return _samsung();
          },
          hostName: 'localhost',
        ),
        _samsungFacts,
      );
      expect(asked, 1);
      expect(
        await readDeviceFacts(
          deviceInfo: () async => throw Exception('no plugin'),
          hostName: 'localhost',
        ),
        const DeviceFacts(
          deviceName: '',
          manufacturer: '',
          model: '',
          modelName: '',
          hostName: 'localhost',
        ),
      );
    });
  });

  group('the default device name on a phone', () {
    // What the core answers for [_samsungFacts] on Android
    // (client/core session::device_name, tested there).
    const signedOut = SessionState(
      kind: SessionKind.signedOut,
      knownAccounts: [],
      deviceName: 'Samsung SM-S921B',
      unsyncedOps: 0,
    );
    const config = CoreConfig(
      appDataDir: '/data/user/0/app.strata/files',
      platform: Platform.android,
      device: _samsungFacts,
      serverUrl: StrataFixtures.serverUrl,
      releaseBuild: true,
    );

    for (final v in variants()) {
      testWidgets('reaches the core and prefills the editable field $v', (
        tester,
      ) async {
        final l10n = lookupAccountsLocalizations(v.locale);
        final app = await boot(
          tester,
          session: signedOut,
          bootstrap: FakeBootstrap(coreConfig: config),
          size: v.size,
          brightness: v.brightness,
          locale: v.locale,
          textScale: v.textScale,
        );
        expect(
          app.fake.calls.first,
          const CoreCall('initCore', {'config': config}),
        );
        expect(find.byType(SignInScreen), findsOneWidget);
        final field = find.widgetWithText(TextField, l10n.fieldDeviceName);
        expect(
          tester.widget<TextField>(field).controller?.text,
          'Samsung SM-S921B',
        );

        // The user can still rename the device; sign-in sends the edit.
        await tester.ensureVisible(field);
        await tester.enterText(field, 'Shawket’s phone');
        await tester.enterText(
          find.widgetWithText(TextField, l10n.fieldUsername),
          'shawket',
        );
        await tapVisible(
          tester,
          find.widgetWithText(FilledButton, l10n.signInTitle),
        );
        expect(
          app.fake.calls.last,
          const CoreCall('signIn', {
            'request': SignInRequest(
              username: 'shawket',
              password: '',
              deviceName: 'Shawket’s phone',
            ),
          }),
        );
        expectNoErrors(tester);
      });
    }
  });
}
