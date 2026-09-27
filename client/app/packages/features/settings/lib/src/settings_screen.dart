import 'dart:async';

import 'package:flutter/material.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_accounts/strata_accounts.dart'
    show showAccountSheet, signOutFlow;
import 'package:strata_settings/src/l10n.dart';
import 'package:strata_settings/src/sections.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// A settings section (also its route segment, `SettingsSection.name`).
enum SettingsSection {
  /// Account: profile, password, language, time zone.
  account,

  /// Devices and this device's reminders switch.
  devices,

  /// Reminders on this device.
  reminders,

  /// AI status and budgets.
  ai,

  /// Integrity warnings.
  integrity,

  /// Export and import.
  data,

  /// Sync status.
  sync,

  /// Admin → Users (admins).
  admin,

  /// About and licences.
  about;

  /// The section named [name], if any.
  static SettingsSection? tryParse(String? name) {
    for (final section in values) {
      if (section.name == name) return section;
    }
    return null;
  }
}

/// Settings (§11 screen 13): on compact a list of sections, each opening
/// full-screen; on medium and expanded the section navigation (224) next to
/// the selected section (SCREEN_SPEC AdminUsersExpanded shows the frame).
class SettingsScreen extends ConsumerWidget {
  /// Creates the screen showing [section] (`null`: the compact list, or the
  /// account section on wider windows).
  const new({
    super.key,
    this.section,
    this.onSelectSection,
    this.adminPane,
    this.onOpenConflict,
  });

  /// The icon that represents this feature.
  static const IconData icon = Icons.settings_outlined;

  /// The section shown.
  final SettingsSection? section;

  /// Navigates to a section (`null`: back to the list).
  final ValueChanged<SettingsSection?>? onSelectSection;

  /// Admin → Users, rendered in the content pane on wide windows.
  final Widget? adminPane;

  /// Opens a sync conflict.
  final ValueChanged<String>? onOpenConflict;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final settings = ref.watch(settingsProvider);
    final compact = SizeClass.of(context) == SizeClass.compact;
    final current = section;
    final content = switch (settings) {
      AsyncData(:final value) => _Loaded(
        view: value,
        section: compact ? current : current ?? SettingsSection.account,
        compact: compact,
        onSelect: onSelectSection,
        adminPane: adminPane,
        onOpenConflict: onOpenConflict,
      ),
      AsyncError(:final error) => StrataEmptyState(
        icon: Icons.error_outline,
        title: l10n.loadFailed,
        message: l10n.failure(error),
      ),
      _ => Center(
        child: Semantics(
          label: l10n.loading,
          child: const CircularProgressIndicator(),
        ),
      ),
    };
    if (compact && current != null) {
      return Scaffold(
        appBar: AppBar(
          leading: BackButton(
            onPressed: onSelectSection == null
                ? null
                : () => onSelectSection?.call(null),
          ),
          title: Text(sectionTitle(l10n, current)),
        ),
        body: content,
      );
    }
    return Material(color: context.strataColors.background, child: content);
  }
}

/// The title of [section].
String sectionTitle(SettingsLocalizations l10n, SettingsSection section) =>
    switch (section) {
      SettingsSection.account => l10n.sectionAccount,
      SettingsSection.devices => l10n.sectionDevices,
      SettingsSection.reminders => l10n.sectionReminders,
      SettingsSection.ai => l10n.sectionAi,
      SettingsSection.integrity => l10n.sectionIntegrity,
      SettingsSection.data => l10n.sectionData,
      SettingsSection.sync => l10n.sectionSync,
      SettingsSection.admin => l10n.sectionAdmin,
      SettingsSection.about => l10n.sectionAbout,
    };

IconData _sectionIcon(SettingsSection section) => switch (section) {
  SettingsSection.account => Icons.person_outline,
  SettingsSection.devices => Icons.devices_outlined,
  SettingsSection.reminders => Icons.notifications_outlined,
  SettingsSection.ai => Icons.auto_awesome_outlined,
  SettingsSection.integrity => Icons.health_and_safety_outlined,
  SettingsSection.data => Icons.import_export,
  SettingsSection.sync => Icons.sync,
  SettingsSection.admin => Icons.admin_panel_settings_outlined,
  SettingsSection.about => Icons.info_outline,
};

class _Loaded extends ConsumerWidget {
  const new({
    required this.view,
    required this.section,
    required this.compact,
    required this.onSelect,
    required this.adminPane,
    required this.onOpenConflict,
  });

  final SettingsView view;
  final SettingsSection? section;
  final bool compact;
  final ValueChanged<SettingsSection?>? onSelect;
  final Widget? adminPane;
  final ValueChanged<String>? onOpenConflict;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final current = section;
    Widget pane(SettingsSection section) => SectionContent(
      section: section,
      view: view,
      adminPane: adminPane,
      onOpenAdmin: onSelect == null
          ? null
          : () => onSelect?.call(SettingsSection.admin),
      onOpenConflict: onOpenConflict,
    );
    if (compact) {
      return current == null
          ? _SettingsList(view: view, onSelect: onSelect)
          : pane(current);
    }
    final colors = context.strataColors;
    return Row(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SizedBox(
          width: 224,
          child: Material(
            color: colors.surface,
            child: _SettingsNav(
              view: view,
              selected: current,
              onSelect: onSelect,
            ),
          ),
        ),
        VerticalDivider(width: 1, color: colors.border),
        Expanded(child: pane(current ?? SettingsSection.account)),
      ],
    );
  }
}

List<(String, List<SettingsSection>)> _groups(
  SettingsLocalizations l10n,
  SettingsView view,
) => [
  (
    l10n.groupYou,
    const [
      SettingsSection.account,
      SettingsSection.devices,
      SettingsSection.reminders,
    ],
  ),
  (
    l10n.groupApp,
    const [
      SettingsSection.ai,
      SettingsSection.integrity,
      SettingsSection.data,
      SettingsSection.sync,
      SettingsSection.about,
    ],
  ),
  if (view.admin == Availability.available)
    (l10n.groupAdmin, const [SettingsSection.admin]),
];

class _SettingsNav extends StatelessWidget {
  const new({
    required this.view,
    required this.selected,
    required this.onSelect,
  });

  final SettingsView view;
  final SettingsSection? selected;
  final ValueChanged<SettingsSection?>? onSelect;

  @override
  Widget build(BuildContext context) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final current = selected ?? SettingsSection.account;
    return Semantics(
      container: true,
      label: l10n.settingsNav,
      explicitChildNodes: true,
      child: ListView(
        padding: const EdgeInsets.all(StrataSpacing.s3),
        children: [
          Padding(
            padding: const EdgeInsets.all(StrataSpacing.s2),
            child: Semantics(
              header: true,
              container: true,
              child: Text(l10n.title, style: text.titleSmall),
            ),
          ),
          for (final (group, sections) in _groups(l10n, view)) ...[
            StrataSectionHeader(
              title: group,
              padding: const EdgeInsetsDirectional.fromSTEB(
                StrataSpacing.s2,
                StrataSpacing.s4,
                StrataSpacing.s2,
                StrataSpacing.s1,
              ),
            ),
            for (final section in sections)
              Padding(
                padding: const EdgeInsets.only(bottom: 2),
                child: Semantics(
                  selected: section == current,
                  child: ListTile(
                    dense: true,
                    minTileHeight: StrataLayout.minTapTarget(context),
                    selected: section == current,
                    selectedTileColor: colors.accentTint,
                    selectedColor: colors.accentText,
                    shape: const RoundedRectangleBorder(
                      borderRadius: StrataRadii.inputRadius,
                    ),
                    leading: Icon(_sectionIcon(section), size: 20),
                    title: Text(sectionTitle(l10n, section), style: text.label),
                    onTap: onSelect == null
                        ? null
                        : () => onSelect?.call(section),
                  ),
                ),
              ),
          ],
        ],
      ),
    );
  }
}

class _SettingsList extends ConsumerWidget {
  const new({required this.view, required this.onSelect});

  final SettingsView view;
  final ValueChanged<SettingsSection?>? onSelect;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.settingsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final account = view.account;
    String? trailing(SettingsSection section) => switch (section) {
      SettingsSection.account => l10n.languageName(account.uiLanguage),
      SettingsSection.devices => l10n.availability(view.devices),
      SettingsSection.ai => l10n.availability(view.ai),
      SettingsSection.integrity => l10n.availability(view.integrity),
      SettingsSection.data => l10n.availability(view.export_),
      _ => null,
    };
    return ListView(
      padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
      children: [
        ListTile(
          minTileHeight: 72,
          leading: CircleAvatar(
            backgroundColor: colors.accentTint,
            foregroundColor: colors.accentText,
            child: const Icon(Icons.person_outline),
          ),
          title: Text(account.displayName, style: text.bodyStrong),
          subtitle: Text(
            l10n.atUsernameRole(
              username: account.username,
              role: l10n.roleName(account.role),
            ),
            style: text.caption.copyWith(color: colors.text2),
          ),
          trailing: Tooltip(
            message: l10n.openAccount,
            child: const Icon(Icons.chevron_right),
          ),
          onTap: () => unawaited(
            showAccountSheet(
              context,
              onOpenDevices: onSelect == null
                  ? null
                  : () => onSelect?.call(SettingsSection.devices),
              onOpenAdminUsers: onSelect == null
                  ? null
                  : () => onSelect?.call(SettingsSection.admin),
            ),
          ),
        ),
        for (final (group, sections) in _groups(l10n, view)) ...[
          StrataSectionHeader(title: group),
          for (final section in sections)
            ListTile(
              minTileHeight: 52,
              leading: Icon(_sectionIcon(section), color: colors.text2),
              title: Text(sectionTitle(l10n, section), style: text.body),
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  if (trailing(section) case final value?)
                    Text(
                      value,
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                  const Icon(Icons.chevron_right),
                ],
              ),
              onTap: onSelect == null ? null : () => onSelect?.call(section),
            ),
        ],
        const Divider(),
        ListTile(
          minTileHeight: 52,
          leading: Icon(Icons.logout, color: colors.dangerText),
          title: Text(
            l10n.signOut,
            style: text.body.copyWith(color: colors.dangerText),
          ),
          onTap: () => unawaited(signOutFlow(context, ref)),
        ),
      ],
    );
  }
}
