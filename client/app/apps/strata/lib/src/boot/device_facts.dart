import 'dart:io' as io;

import 'package:device_info_plus/device_info_plus.dart';
import 'package:strata_state/strata_state.dart' show DeviceFacts;

/// Reads the platform's device information (`device_info_plus`'s
/// `DeviceInfoPlugin().deviceInfo`, the subtype of the running platform).
typedef DeviceInfoSource = Future<BaseDeviceInfo> Function();

/// This device's raw facts for the core's default device name (PLAN §12.7):
/// what `device_info_plus` (allowed in the app shell only) and the host name
/// report, unchanged. The Rust core picks, cleans and formats the name (L15);
/// a platform that fails to answer leaves only the host name.
Future<DeviceFacts> readDeviceFacts({
  DeviceInfoSource? deviceInfo,
  String? hostName,
}) async {
  final host = hostName ?? io.Platform.localHostname;
  try {
    return deviceFactsOf(
      await (deviceInfo ?? () => DeviceInfoPlugin().deviceInfo)(),
      hostName: host,
    );
  } on Exception {
    return deviceFactsOf(null, hostName: host);
  }
}

/// The fields of [info] the core reads, per platform; empty when the
/// platform has no such fact.
DeviceFacts deviceFactsOf(BaseDeviceInfo? info, {required String hostName}) =>
    switch (info) {
      AndroidDeviceInfo(:final name, :final manufacturer, :final model) =>
        DeviceFacts(
          deviceName: name,
          manufacturer: manufacturer,
          model: model,
          modelName: '',
          hostName: hostName,
        ),
      IosDeviceInfo(:final name, :final model, :final modelName) => DeviceFacts(
        deviceName: name,
        manufacturer: '',
        model: model,
        modelName: modelName,
        hostName: hostName,
      ),
      MacOsDeviceInfo(:final computerName, :final model, :final modelName) =>
        DeviceFacts(
          deviceName: computerName,
          manufacturer: '',
          model: model,
          modelName: modelName,
          hostName: hostName,
        ),
      WindowsDeviceInfo(:final computerName) => DeviceFacts(
        deviceName: computerName,
        manufacturer: '',
        model: '',
        modelName: '',
        hostName: hostName,
      ),
      _ => DeviceFacts(
        deviceName: '',
        manufacturer: '',
        model: '',
        modelName: '',
        hostName: hostName,
      ),
    };
