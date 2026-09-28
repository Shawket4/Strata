import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/strata_accounts.dart' show signOutFlow;
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_settings/src/l10n.dart';
import 'package:strata_settings/src/settings_screen.dart';
import 'package:strata_settings/src/time_zone_picker.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_sync/strata_sync.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// The content of one settings section, rendered from [SettingsView].
class SectionContent extends StatelessWidget {
  /// Creates the content of [section].
  const new({
    required this.section,
    required this.view,
    super.key,
    this.adminPane,
    this.onOpenAdmin,
    this.onOpenConflict,
  });

  /// The section.
  final SettingsSection section;

  /// The core's settings.
  final SettingsView view;

  /// Admin → Users, embedded on wide windows.
  final Widget? adminPane;

  /// Opens Admin → Users (compact).
  final VoidCallback? onOpenAdmin;

  /// Opens a sync conflict.
  final ValueChanged<String>? onOpenConflict;

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
    final wide = SizeClass.of(context) != SizeClass.compact;
    final admin = adminPane;
    if (section == SettingsSection.sync) {
      return SyncStatusPanel(
        surface: SyncSurface.page,
        onOpenConflict: onOpenConflict,
      );
    }
    if (section == SettingsSection.admin && wide && admin != null) {
      return admin;
    }
    final children = switch (section) {
      SettingsSection.account => [AccountSection(view: view)],
      SettingsSection.devices => [DevicesSection(view: view)],
      SettingsSection.reminders => [RemindersSection(setting: view.reminders)],
      SettingsSection.ai => [AiSection(view: view)],
      SettingsSection.integrity => [IntegritySection(view: view)],
      SettingsSection.data => [_DataSection(availability: view.export_)],
      SettingsSection.admin => [
        _AvailabilityCard(
          availability: view.admin,
          icon: Icons.admin_panel_settings_outlined,
          body: l10n.adminBody,
          action: view.admin == Availability.available ? onOpenAdmin : null,
          actionLabel: l10n.openAdminUsers,
        ),
      ],
      SettingsSection.about => [const _AboutSection()],
      SettingsSection.sync => const <Widget>[],
    };
    return ListView(
      padding: EdgeInsets.all(wide ? StrataSpacing.s8 : StrataSpacing.s4),
      children: [
        if (wide) ...[
          Semantics(
            header: true,
            container: true,
            child: Text(
              sectionTitle(l10n, section),
              style: context.strataText.display,
            ),
          ),
          const SizedBox(height: StrataSpacing.s5),
        ],
        Align(
          alignment: AlignmentDirectional.topStart,
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 640),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: children,
            ),
          ),
        ),
      ],
    );
  }
}

/// Runs a settings [intent] on the core and shows [done] (when given) or the
/// failure in a snack bar.
Future<void> _run(
  BuildContext context,
  WidgetRef ref,
  Future<Object?> Function(CoreApi core) intent, {
  String? done,
}) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final l10n = context.settingsL10n;
  try {
    await intent(ref.read(coreApiProvider));
    if (done != null) {
      messenger
        ?..hideCurrentSnackBar()
        ..showSnackBar(SnackBar(content: Text(done)));
    }
  } on Object catch (error) {
    messenger
      ?..hideCurrentSnackBar()
      ..showSnackBar(SnackBar(content: Text(l10n.failure(error))));
  }
}

/// Queues the failed AI jobs again and says how many, or the failure.
Future<void> _retryFailedJobs(BuildContext context, WidgetRef ref) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final l10n = context.settingsL10n;
  String message;
  try {
    final count = await ref.read(coreApiProvider).retryFailedJobs();
    message = l10n.aiRetried(count: count);
  } on Object catch (error) {
    message = l10n.failure(error);
  }
  messenger
    ?..hideCurrentSnackBar()
    ..showSnackBar(SnackBar(content: Text(message)));
}

/// Asks for one text value ([initial] prefilled); `null` when cancelled.
Future<String?> _askText(
  BuildContext context, {
  required String title,
  required String label,
  required String initial,
}) => showDialog<String>(
  context: context,
  builder: (_) => _TextDialog(title: title, label: label, initial: initial),
);

class _TextDialog extends HookWidget {
  const new({required this.title, required this.label, required this.initial});

  final String title;
  final String label;
  final String initial;

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
    final controller = useTextEditingController(text: initial);
    return AlertDialog(
      title: Text(title),
      scrollable: true,
      content: TextField(
        controller: controller,
        autofocus: true,
        decoration: InputDecoration(labelText: label),
        onSubmitted: (value) => Navigator.of(context).pop(value),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(controller.text),
          child: Text(l10n.save),
        ),
      ],
    );
  }
}

/// A bordered group of rows.
class _Group extends StatelessWidget {
  const new({required this.children, this.title});

  final String? title;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final heading = title;
    return Padding(
      padding: const EdgeInsets.only(bottom: StrataSpacing.s5),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          if (heading != null)
            StrataSectionHeader(
              title: heading,
              padding: const EdgeInsetsDirectional.only(
                bottom: StrataSpacing.s2,
              ),
            ),
          Material(
            color: colors.surface,
            clipBehavior: Clip.antiAlias,
            shape: RoundedRectangleBorder(
              borderRadius: StrataRadii.cardRadius,
              side: BorderSide(color: colors.border),
            ),
            child: Column(
              children: [
                for (final (i, child) in children.indexed) ...[
                  if (i > 0) Divider(height: 1, color: colors.border),
                  child,
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _ValueRow extends StatelessWidget {
  const new({
    required this.label,
    required this.value,
    this.mono = false,
    this.onEdit,
    this.editLabel,
  });

  final String label;
  final String value;
  final bool mono;
  final VoidCallback? onEdit;
  final String? editLabel;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final edit = onEdit;
    return Padding(
      padding: EdgeInsetsDirectional.fromSTEB(
        StrataSpacing.s4,
        edit == null ? StrataSpacing.s3 : 0,
        edit == null ? StrataSpacing.s4 : 0,
        edit == null ? StrataSpacing.s3 : 0,
      ),
      child: Row(
        children: [
          Expanded(
            child: MergeSemantics(
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Expanded(child: Text(label, style: text.body)),
                  const SizedBox(width: StrataSpacing.s3),
                  Flexible(
                    child: Align(
                      alignment: AlignmentDirectional.centerEnd,
                      child: Text(
                        value,
                        textAlign: TextAlign.end,
                        textDirection: mono ? TextDirection.ltr : null,
                        style: (mono ? text.monoSmall : text.bodySmall)
                            .copyWith(color: colors.text2),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
          if (edit != null)
            IconButton(
              onPressed: edit,
              tooltip: editLabel ?? context.settingsL10n.edit,
              icon: const Icon(Icons.edit_outlined, size: 20),
            ),
        ],
      ),
    );
  }
}

/// Settings → Account: profile (display name), language, time zone (each
/// saved by the core, which rebuilds every label), password change, sign
/// out.
class AccountSection extends HookConsumerWidget {
  /// Creates the section.
  const new({required this.view, super.key});

  /// The core's settings.
  final SettingsView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final account = view.account;
    final current = useTextEditingController();
    final next = useTextEditingController();
    Future<void> editName() async {
      final name = await _askText(
        context,
        title: l10n.editDisplayNameTitle,
        label: l10n.displayName,
        initial: account.displayName,
      );
      if (name == null || !context.mounted) return;
      await _run(
        context,
        ref,
        (core) => core.setDisplayName(name: name),
        done: l10n.saved,
      );
    }

    Future<void> editZone() async {
      final zone = await showTimeZonePicker(context);
      if (zone == null || !context.mounted) return;
      await _run(
        context,
        ref,
        (core) => core.setTimezone(iana: zone),
        done: l10n.saved,
      );
    }

    Future<void> changePassword() async {
      await _run(
        context,
        ref,
        (core) => core.changePassword(current: current.text, new_: next.text),
        done: l10n.passwordChanged,
      );
      current.clear();
      next.clear();
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _Group(
          children: [
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: Row(
                children: [
                  StrataAvatar(initials: account.initials, size: 48),
                  const SizedBox(width: StrataSpacing.s3),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(account.displayName, style: text.bodyStrong),
                        Text(
                          l10n.atUsernameRole(
                            username: account.username,
                            role: l10n.roleName(account.role),
                          ),
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
            _ValueRow(
              label: l10n.displayName,
              value: account.displayName,
              onEdit: () => unawaited(editName()),
              editLabel: l10n.editDisplayNameTitle,
            ),
            _ValueRow(
              label: l10n.username,
              value: account.username,
              mono: true,
            ),
            _ValueRow(label: l10n.role, value: l10n.roleName(account.role)),
            _ValueRow(label: l10n.server, value: account.serverUrl, mono: true),
          ],
        ),
        _Group(
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s4),
              child: Row(
                children: [
                  Expanded(child: Text(l10n.language, style: text.body)),
                  Flexible(
                    child: Semantics(
                      label: l10n.language,
                      container: true,
                      child: DropdownButtonHideUnderline(
                        child: DropdownButton<String>(
                          value: account.uiLanguage,
                          isExpanded: true,
                          alignment: AlignmentDirectional.centerEnd,
                          items: [
                            for (final code in const ['en', 'ar'])
                              DropdownMenuItem(
                                value: code,
                                alignment: AlignmentDirectional.centerEnd,
                                child: Text(
                                  l10n.languageName(code),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: text.bodySmall.copyWith(
                                    color: colors.text,
                                  ),
                                ),
                              ),
                          ],
                          onChanged: (code) {
                            if (code == null || code == account.uiLanguage) {
                              return;
                            }
                            unawaited(
                              _run(
                                context,
                                ref,
                                (core) => core.setUiLanguage(code: code),
                              ),
                            );
                          },
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
            _ValueRow(
              label: l10n.timezone,
              value: account.timezone,
              mono: true,
              onEdit: () => unawaited(editZone()),
              editLabel: l10n.editTimezoneTitle,
            ),
          ],
        ),
        _Group(
          title: l10n.passwordGroup,
          children: [
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    controller: current,
                    obscureText: true,
                    decoration: InputDecoration(
                      labelText: l10n.currentPassword,
                    ),
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  TextField(
                    controller: next,
                    obscureText: true,
                    decoration: InputDecoration(labelText: l10n.newPassword),
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  Align(
                    alignment: AlignmentDirectional.centerEnd,
                    child: FilledButton(
                      onPressed: () => unawaited(changePassword()),
                      child: Text(l10n.changePasswordTitle),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
        OutlinedButton.icon(
          onPressed: () => unawaited(signOutFlow(context, ref)),
          icon: const Icon(Icons.logout),
          label: Text(l10n.signOut),
        ),
      ],
    );
  }
}

/// The icon of a device platform (`android`, `ios`, `macos`, `windows`,
/// `linux`).
IconData platformIcon(String platform) => switch (platform) {
  'android' || 'ios' => Icons.smartphone_outlined,
  'macos' || 'windows' || 'linux' => Icons.laptop_outlined,
  _ => Icons.devices_other_outlined,
};

/// Settings → Devices: every signed-in device (the core's order) with its
/// last-seen and sign-in labels, its reminders switch ("Deliver to"),
/// rename and sign-out.
class DevicesSection extends ConsumerWidget {
  /// Creates the section.
  const new({required this.view, super.key});

  /// The core's settings.
  final SettingsView view;

  Future<void> _rename(
    BuildContext context,
    WidgetRef ref,
    DeviceItem device,
  ) async {
    final l10n = context.settingsL10n;
    final name = await _askText(
      context,
      title: l10n.renameTitle,
      label: l10n.rename,
      initial: device.name,
    );
    if (name == null || !context.mounted) return;
    await _run(
      context,
      ref,
      (core) => core.renameDevice(id: device.id, name: name),
    );
  }

  Future<void> _revoke(
    BuildContext context,
    WidgetRef ref,
    DeviceItem device,
  ) async {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialog) => AlertDialog(
        icon: Icon(Icons.logout, color: colors.dangerText),
        title: Text(l10n.revokeTitle(name: device.name)),
        content: Text(l10n.revokeBody),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialog).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: colors.dangerTint,
              foregroundColor: colors.dangerText,
            ),
            onPressed: () => Navigator.of(dialog).pop(true),
            child: Text(l10n.revoke),
          ),
        ],
      ),
    );
    if (confirmed != true || !context.mounted) return;
    await _run(context, ref, (core) => core.revokeDevice(id: device.id));
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final refreshed = view.refreshedLabel;
    if (view.devices != Availability.available && view.deviceList.isEmpty) {
      return _AvailabilityCard(
        availability: view.devices,
        icon: Icons.devices_outlined,
        body: l10n.sectionDevices,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                refreshed ?? '',
                style: text.caption.copyWith(color: colors.text2),
              ),
            ),
            IconButton(
              tooltip: l10n.refresh,
              onPressed: () => unawaited(
                _run(context, ref, (core) => core.refreshSettings()),
              ),
              icon: const Icon(Icons.refresh),
            ),
          ],
        ),
        if (view.deviceList.isEmpty)
          Text(
            l10n.noDevices,
            style: text.bodySmall.copyWith(color: colors.text2),
          )
        else
          _Group(
            children: [
              for (final device in view.deviceList)
                Padding(
                  padding: const EdgeInsetsDirectional.fromSTEB(
                    StrataSpacing.s4,
                    StrataSpacing.s3,
                    StrataSpacing.s2,
                    StrataSpacing.s3,
                  ),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Row(
                        children: [
                          Icon(
                            platformIcon(device.platform),
                            color: colors.text2,
                          ),
                          const SizedBox(width: StrataSpacing.s3),
                          Expanded(
                            child: MergeSemantics(
                              child: Column(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Wrap(
                                    spacing: StrataSpacing.s2,
                                    crossAxisAlignment:
                                        WrapCrossAlignment.center,
                                    children: [
                                      Text(device.name, style: text.bodyStrong),
                                      if (device.isThisDevice)
                                        StatusPill(
                                          label: l10n.thisDeviceBadge,
                                          tone: StatusTone.info,
                                        ),
                                    ],
                                  ),
                                  Text(
                                    l10n.deviceDetail(
                                      lastSeen: device.lastSeenLabel,
                                      signedIn: device.signedInLabel,
                                    ),
                                    style: text.caption.copyWith(
                                      color: colors.text2,
                                    ),
                                  ),
                                ],
                              ),
                            ),
                          ),
                          IconButton(
                            tooltip: l10n.renameDevice(name: device.name),
                            onPressed: () =>
                                unawaited(_rename(context, ref, device)),
                            icon: const Icon(Icons.edit_outlined, size: 20),
                          ),
                          if (!device.isThisDevice)
                            IconButton(
                              tooltip: l10n.revokeDevice(name: device.name),
                              onPressed: () =>
                                  unawaited(_revoke(context, ref, device)),
                              icon: Icon(
                                Icons.logout,
                                size: 20,
                                color: colors.dangerText,
                              ),
                            ),
                        ],
                      ),
                      SwitchListTile(
                        contentPadding: const EdgeInsetsDirectional.only(
                          start: StrataSpacing.s8 + StrataSpacing.s1,
                          end: StrataSpacing.s2,
                        ),
                        dense: true,
                        value: device.remindersEnabled,
                        title: Text(
                          l10n.deviceReminders(name: device.name),
                          style: text.bodySmall,
                        ),
                        onChanged: (enabled) => unawaited(
                          _run(
                            context,
                            ref,
                            (core) => core.setDeviceReminders(
                              id: device.id,
                              enabled: enabled,
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                ),
            ],
          ),
      ],
    );
  }
}

class _RemindersSwitch extends ConsumerWidget {
  const new({required this.setting});

  final RemindersSetting setting;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    return SwitchListTile(
      value: setting.enabled,
      title: Text(l10n.remindersOnDevice),
      subtitle: Text(l10n.remindersOnDeviceHelp),
      onChanged: (enabled) => unawaited(
        ref.read(coreApiProvider).setRemindersEnabled(enabled: enabled),
      ),
    );
  }
}

/// The snooze lengths offered (minutes).
const List<int> snoozeChoices = [5, 10, 15, 30, 60];

/// Reminders on this device (§12.5b, SCREEN_SPEC ReminderNotifications
/// settings column): the per-device switch, the platform permission, how
/// reminders are delivered here and how many are scheduled, the default
/// time, the snooze length and quiet hours (each saved by the core).
class RemindersSection extends HookConsumerWidget {
  /// Creates the section for [setting].
  const new({required this.setting, super.key});

  /// The core's reminder settings of this device.
  final RemindersSetting setting;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final time = useTextEditingController(text: setting.defaultTime);
    final from = useTextEditingController(text: setting.quietFrom);
    final until = useTextEditingController(text: setting.quietUntil);
    void saveQuiet({required bool enabled}) => unawaited(
      _run(
        context,
        ref,
        (core) => core.setQuietHours(
          enabled: enabled,
          from: from.text,
          until: until.text,
        ),
      ),
    );
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (setting.permission == NotificationPermission.denied)
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s4),
            child: Semantics(
              container: true,
              liveRegion: true,
              child: Container(
                padding: const EdgeInsets.all(StrataSpacing.s4),
                decoration: BoxDecoration(
                  color: colors.warningTint,
                  borderRadius: StrataRadii.cardRadius,
                ),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Icon(
                      Icons.notifications_off_outlined,
                      color: colors.warningText,
                    ),
                    const SizedBox(width: StrataSpacing.s3),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            l10n.remindersOffPermission,
                            style: text.bodyStrong.copyWith(
                              color: colors.warningText,
                            ),
                          ),
                          Text(
                            l10n.remindersOffPermissionBody,
                            style: text.bodySmall.copyWith(color: colors.text),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        _Group(
          children: [
            _RemindersSwitch(setting: setting),
            ListTile(
              leading: Icon(switch (setting.permission) {
                NotificationPermission.granted =>
                  Icons.notifications_active_outlined,
                NotificationPermission.denied =>
                  Icons.notifications_off_outlined,
                NotificationPermission.unknown => Icons.notifications_none,
              }),
              title: Text(switch (setting.permission) {
                NotificationPermission.granted =>
                  l10n.remindersPermissionGranted,
                NotificationPermission.denied => l10n.remindersOffPermission,
                NotificationPermission.unknown =>
                  l10n.remindersPermissionUnknown,
              }),
              subtitle: Text(switch (setting.mode) {
                NotificationMode.osScheduled => l10n.deliveryOs,
                NotificationMode.whileRunning => l10n.deliveryWhileRunning,
              }),
            ),
            ListTile(
              leading: const Icon(Icons.alarm_outlined),
              title: Text(l10n.scheduledCount(count: setting.scheduled)),
            ),
          ],
        ),
        _Group(
          children: [
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Text(l10n.defaultTime, style: text.body),
                  Text(
                    l10n.defaultTimeHelp,
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                  const SizedBox(height: StrataSpacing.s2),
                  TextField(
                    controller: time,
                    textDirection: TextDirection.ltr,
                    style: text.mono,
                    decoration: InputDecoration(labelText: l10n.timeField),
                    onSubmitted: (value) => unawaited(
                      _run(
                        context,
                        ref,
                        (core) => core.setDefaultReminderTime(time: value),
                        done: l10n.saved,
                      ),
                    ),
                  ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s4),
              child: Row(
                children: [
                  Expanded(child: Text(l10n.snooze, style: text.body)),
                  Flexible(
                    child: Semantics(
                      label: l10n.snooze,
                      container: true,
                      child: DropdownButtonHideUnderline(
                        child: DropdownButton<int>(
                          value: snoozeChoices.contains(setting.snoozeMinutes)
                              ? setting.snoozeMinutes
                              : null,
                          isExpanded: true,
                          hint: Text(
                            l10n.snoozeMinutes(count: setting.snoozeMinutes),
                          ),
                          items: [
                            for (final minutes in snoozeChoices)
                              DropdownMenuItem(
                                value: minutes,
                                child: Text(
                                  l10n.snoozeMinutes(count: minutes),
                                  maxLines: 1,
                                  overflow: TextOverflow.ellipsis,
                                  style: text.bodySmall.copyWith(
                                    color: colors.text,
                                  ),
                                ),
                              ),
                          ],
                          onChanged: (minutes) {
                            if (minutes == null) return;
                            unawaited(
                              _run(
                                context,
                                ref,
                                (core) =>
                                    core.setSnoozeMinutes(minutes: minutes),
                              ),
                            );
                          },
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
        _Group(
          children: [
            SwitchListTile(
              value: setting.quietEnabled,
              title: Text(l10n.quietHours),
              subtitle: Text(l10n.quietHoursHelp),
              onChanged: (enabled) => saveQuiet(enabled: enabled),
            ),
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: Wrap(
                spacing: StrataSpacing.s3,
                runSpacing: StrataSpacing.s3,
                children: [
                  for (final (controller, label) in [
                    (from, l10n.quietFrom),
                    (until, l10n.quietUntil),
                  ])
                    SizedBox(
                      width: 200,
                      child: TextField(
                        controller: controller,
                        textDirection: TextDirection.ltr,
                        style: text.mono,
                        decoration: InputDecoration(labelText: label),
                        onSubmitted: (_) =>
                            saveQuiet(enabled: setting.quietEnabled),
                      ),
                    ),
                ],
              ),
            ),
          ],
        ),
        Text(
          l10n.remindersSyncNote,
          style: text.caption.copyWith(color: colors.text2),
        ),
      ],
    );
  }
}

/// Settings → AI: the server's AI status (provider, pause, queue, budget
/// meter, search index progress) and, when AI jobs failed, Retry.
class AiSection extends ConsumerWidget {
  /// Creates the section.
  const new({required this.view, super.key});

  /// The core's settings.
  final SettingsView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final status = view.aiStatus;
    if (status == null) {
      return _AvailabilityCard(
        availability: view.ai,
        icon: Icons.auto_awesome_outlined,
        body: l10n.aiBody,
      );
    }
    final provider = status.provider;
    final paused = status.pausedLabel;
    final embeddings = status.embeddingPercent;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        if (paused != null)
          Padding(
            padding: const EdgeInsets.only(bottom: StrataSpacing.s4),
            child: Semantics(
              container: true,
              liveRegion: true,
              child: Container(
                padding: const EdgeInsets.all(StrataSpacing.s4),
                decoration: BoxDecoration(
                  color: colors.warningTint,
                  borderRadius: StrataRadii.cardRadius,
                ),
                child: Row(
                  children: [
                    Icon(Icons.pause_circle_outline, color: colors.warningText),
                    const SizedBox(width: StrataSpacing.s3),
                    Expanded(
                      child: Text(
                        paused,
                        style: text.bodyStrong.copyWith(
                          color: colors.warningText,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        _Group(
          children: [
            ListTile(
              leading: Icon(
                status.enabled
                    ? Icons.auto_awesome_outlined
                    : Icons.block_outlined,
                color: colors.text2,
              ),
              title: Text(
                !status.enabled
                    ? l10n.aiOff
                    : provider == null
                    ? l10n.aiOnNoProvider
                    : l10n.aiOn(provider: provider),
                style: text.bodyStrong,
              ),
              subtitle: Text(l10n.aiQueue(count: status.queueDepth)),
            ),
            if (status.failedJobs > 0)
              ListTile(
                leading: Icon(Icons.error_outline, color: colors.warningText),
                title: Text(
                  l10n.aiFailedJobs(count: status.failedJobs),
                  style: text.body,
                ),
                trailing: TextButton(
                  onPressed: () => _retryFailedJobs(context, ref),
                  child: Text(l10n.aiRetryFailed),
                ),
              ),
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: MergeSemantics(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    Row(
                      children: [
                        Expanded(child: Text(l10n.aiBudget, style: text.body)),
                        Flexible(
                          child: Text(
                            status.budgetLabel,
                            textAlign: TextAlign.end,
                            style: text.bodySmall.copyWith(color: colors.text2),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: StrataSpacing.s2),
                    LinearProgressIndicator(
                      value: status.budgetUsedPercent / 100,
                      minHeight: 6,
                      borderRadius: StrataRadii.pillRadius,
                      color: colors.accent,
                      backgroundColor: colors.surface2,
                    ),
                  ],
                ),
              ),
            ),
            if (embeddings != null)
              Padding(
                padding: const EdgeInsets.all(StrataSpacing.s4),
                child: MergeSemantics(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Text(
                        l10n.aiEmbeddings(percent: embeddings),
                        style: text.body,
                      ),
                      const SizedBox(height: StrataSpacing.s2),
                      LinearProgressIndicator(
                        value: embeddings / 100,
                        minHeight: 6,
                        borderRadius: StrataRadii.pillRadius,
                        backgroundColor: colors.surface2,
                      ),
                    ],
                  ),
                ),
              ),
          ],
        ),
        Text(
          l10n.aiThresholdsNote,
          style: text.caption.copyWith(color: colors.text2),
        ),
      ],
    );
  }
}

/// Settings → Integrity: the server's integrity warnings, newest first (the
/// core's order).
class IntegritySection extends StatelessWidget {
  /// Creates the section.
  const new({required this.view, super.key});

  /// The core's settings.
  final SettingsView view;

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    if (view.integrity != Availability.available &&
        view.integrityWarnings.isEmpty) {
      return _AvailabilityCard(
        availability: view.integrity,
        icon: Icons.health_and_safety_outlined,
        body: l10n.integrityBody,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.only(bottom: StrataSpacing.s3),
          child: Text(
            l10n.integrityBody,
            style: text.bodySmall.copyWith(color: colors.text2),
          ),
        ),
        if (view.integrityWarnings.isEmpty)
          Text(
            l10n.integrityEmpty,
            style: text.bodySmall.copyWith(color: colors.text2),
          )
        else
          _Group(
            children: [
              for (final item in view.integrityWarnings)
                MergeSemantics(
                  child: ListTile(
                    leading: Icon(
                      Icons.report_gmailerrorred_outlined,
                      color: colors.warningText,
                    ),
                    title: Text(l10n.integrityMessage(item)),
                    subtitle: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        if (item.path case final path?)
                          Text(
                            path,
                            textDirection: TextDirection.ltr,
                            style: text.monoSmall.copyWith(color: colors.text2),
                          ),
                        Text(
                          item.createdLabel,
                          style: text.caption.copyWith(color: colors.text2),
                        ),
                      ],
                    ),
                  ),
                ),
            ],
          ),
      ],
    );
  }
}

class _AvailabilityCard extends StatelessWidget {
  const new({
    required this.availability,
    required this.icon,
    required this.body,
    this.action,
    this.actionLabel,
  });

  final Availability availability;
  final IconData icon;
  final String body;
  final VoidCallback? action;
  final String? actionLabel;

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final tone = switch (availability) {
      Availability.available => StatusTone.success,
      Availability.offline => StatusTone.warning,
      Availability.notYetAvailable => StatusTone.info,
      Availability.notAllowed => StatusTone.neutral,
    };
    final detail = switch (availability) {
      Availability.available => null,
      Availability.offline => l10n.offlineBody,
      Availability.notYetAvailable => l10n.notYetBody,
      Availability.notAllowed => l10n.notAllowedBody,
    };
    final open = action;
    final label = actionLabel;
    return _Group(
      children: [
        Padding(
          padding: const EdgeInsets.all(StrataSpacing.s4),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Icon(icon, color: colors.text2),
                  const SizedBox(width: StrataSpacing.s3),
                  Flexible(
                    child: StatusPill(
                      label: l10n.availability(availability),
                      tone: tone,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: StrataSpacing.s3),
              Text(body, style: text.body),
              if (detail != null) ...[
                const SizedBox(height: StrataSpacing.s1),
                Text(
                  detail,
                  style: text.bodySmall.copyWith(color: colors.text2),
                ),
              ],
              if (open != null && label != null) ...[
                const SizedBox(height: StrataSpacing.s3),
                FilledButton(onPressed: open, child: Text(label)),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

/// Settings → Export & import: the vault as a zip (`export_vault`) and a
/// zip of Markdown in (`import_vault`). The paths come from the native file
/// dialogs (`filePickerProvider`); the core reads and writes the files.
class _DataSection extends ConsumerWidget {
  const new({required this.availability});

  final Availability availability;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final enabled = availability == Availability.available;
    Future<void> export() async {
      final path = await ref
          .read(filePickerProvider)
          .saveFile(
            suggestedName: 'strata-vault.zip',
            type: PickedFileType.zip,
          );
      if (path == null || !context.mounted) return;
      final messenger = ScaffoldMessenger.maybeOf(context);
      try {
        final summary = await ref.read(coreApiProvider).exportVault(path: path);
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(
            SnackBar(content: Text(l10n.exportDone(label: summary.label))),
          );
      } on Object catch (error) {
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(l10n.failure(error))));
      }
    }

    Future<void> import() async {
      final path = await ref
          .read(filePickerProvider)
          .openFile(type: PickedFileType.zip);
      if (path == null || !context.mounted) return;
      final messenger = ScaffoldMessenger.maybeOf(context);
      try {
        final summary = await ref.read(coreApiProvider).importVault(path: path);
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(
            SnackBar(
              content: Text(
                l10n.importDone(
                  imported: summary.imported,
                  skipped: summary.skipped,
                ),
              ),
            ),
          );
      } on Object catch (error) {
        messenger
          ?..hideCurrentSnackBar()
          ..showSnackBar(SnackBar(content: Text(l10n.failure(error))));
      }
    }

    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _AvailabilityCard(
          availability: availability,
          icon: Icons.import_export,
          body: l10n.dataBody,
        ),
        Wrap(
          spacing: StrataSpacing.s2,
          runSpacing: StrataSpacing.s2,
          children: [
            FilledButton.icon(
              onPressed: enabled ? () => unawaited(export()) : null,
              icon: const Icon(Icons.download_outlined),
              label: Text(l10n.exportVault),
            ),
            OutlinedButton.icon(
              onPressed: enabled ? () => unawaited(import()) : null,
              icon: const Icon(Icons.upload_outlined),
              label: Text(l10n.importFiles),
            ),
          ],
        ),
      ],
    );
  }
}

class _AboutSection extends StatelessWidget {
  const new();

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const StrataWordmark(fontSize: 32),
        const SizedBox(height: StrataSpacing.s3),
        Text(l10n.aboutBody, style: text.body),
        const SizedBox(height: StrataSpacing.s3),
        Text(
          l10n.fontsNote,
          style: text.bodySmall.copyWith(color: colors.text2),
        ),
        const SizedBox(height: StrataSpacing.s4),
        OutlinedButton.icon(
          onPressed: () => showLicensePage(
            context: context,
            applicationName: context.l10n.appTitle,
          ),
          icon: const Icon(Icons.description_outlined),
          label: Text(l10n.licences),
        ),
      ],
    );
  }
}
