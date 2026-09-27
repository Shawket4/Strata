import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/strata_accounts.dart' show signOutFlow;
import 'package:strata_l10n/strata_l10n.dart';
import 'package:strata_settings/src/l10n.dart';
import 'package:strata_settings/src/settings_screen.dart';
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
      SettingsSection.account => [_AccountSection(view: view)],
      SettingsSection.devices => [_DevicesSection(view: view)],
      SettingsSection.reminders => [RemindersSection(setting: view.reminders)],
      SettingsSection.ai => [
        _AvailabilityCard(
          availability: view.ai,
          icon: Icons.auto_awesome_outlined,
          body: l10n.aiBody,
        ),
      ],
      SettingsSection.integrity => [
        _AvailabilityCard(
          availability: view.integrity,
          icon: Icons.health_and_safety_outlined,
          body: l10n.integrityBody,
        ),
      ],
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
  const new({required this.label, required this.value, this.mono = false});

  final String label;
  final String value;
  final bool mono;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    return MergeSemantics(
      child: Padding(
        padding: const EdgeInsets.symmetric(
          horizontal: StrataSpacing.s4,
          vertical: StrataSpacing.s3,
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Expanded(child: Text(label, style: text.body)),
            const SizedBox(width: StrataSpacing.s3),
            Flexible(
              child: Text(
                value,
                textAlign: TextAlign.end,
                style: (mono ? text.monoSmall : text.bodySmall).copyWith(
                  color: colors.text2,
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// A control whose intent the core does not offer yet: disabled, with a
/// "Not available yet" tooltip (docs/CORE_GAPS.md).
class _NotYet extends StatelessWidget {
  const new({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) =>
      Tooltip(message: context.settingsL10n.notAvailableYet, child: child);
}

class _AccountSection extends ConsumerWidget {
  const new({required this.view});

  final SettingsView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final account = view.account;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _Group(
          children: [
            _ValueRow(label: l10n.displayName, value: account.displayName),
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
            _ValueRow(
              label: l10n.language,
              value: l10n.languageName(account.uiLanguage),
            ),
            _ValueRow(
              label: l10n.timezone,
              value: account.timezone,
              mono: true,
            ),
          ],
        ),
        _Group(
          title: l10n.changePasswordTitle,
          children: [
            Padding(
              padding: const EdgeInsets.all(StrataSpacing.s4),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    enabled: false,
                    obscureText: true,
                    decoration: InputDecoration(
                      labelText: l10n.currentPassword,
                    ),
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  TextField(
                    enabled: false,
                    obscureText: true,
                    decoration: InputDecoration(labelText: l10n.newPassword),
                  ),
                  const SizedBox(height: StrataSpacing.s3),
                  Align(
                    alignment: AlignmentDirectional.centerEnd,
                    child: _NotYet(
                      child: FilledButton(
                        onPressed: null,
                        child: Text(l10n.changePasswordTitle),
                      ),
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

class _DevicesSection extends ConsumerWidget {
  const new({required this.view});

  final SettingsView view;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final device = ref.watch(sessionProvider).value?.deviceName ?? '';
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _Group(
          title: l10n.thisDevice,
          children: [
            ListTile(
              leading: const Icon(Icons.smartphone_outlined),
              title: Text(device, style: text.bodyStrong),
              trailing: _NotYet(
                child: IconButton(
                  onPressed: null,
                  tooltip: l10n.rename,
                  icon: const Icon(Icons.edit_outlined),
                ),
              ),
            ),
            _RemindersSwitch(setting: view.reminders),
          ],
        ),
        if (view.devices == Availability.available)
          Text(
            l10n.deviceListNote,
            style: text.bodySmall.copyWith(color: colors.text2),
          )
        else
          _AvailabilityCard(
            availability: view.devices,
            icon: Icons.devices_outlined,
            body: l10n.deviceListNote,
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

/// Reminders on this device (§12.5b, SCREEN_SPEC ReminderNotifications
/// settings column): the per-device switch, the platform permission, how
/// reminders are delivered here and how many are scheduled.
class RemindersSection extends StatelessWidget {
  /// Creates the section for [setting].
  const new({required this.setting, super.key});

  /// The core's reminder settings of this device.
  final RemindersSetting setting;

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
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
            ListTile(
              title: Text(l10n.defaultTime),
              subtitle: Text(l10n.defaultTimeHelp),
              trailing: Text(
                setting.defaultTime,
                textDirection: TextDirection.ltr,
                style: text.mono,
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

class _DataSection extends StatelessWidget {
  const new({required this.availability});

  final Availability availability;

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
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
            _NotYet(
              child: FilledButton.icon(
                onPressed: null,
                icon: const Icon(Icons.download_outlined),
                label: Text(l10n.exportVault),
              ),
            ),
            _NotYet(
              child: OutlinedButton.icon(
                onPressed: null,
                icon: const Icon(Icons.upload_outlined),
                label: Text(l10n.importFiles),
              ),
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
