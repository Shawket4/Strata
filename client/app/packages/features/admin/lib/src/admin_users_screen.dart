import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:hooks_riverpod/hooks_riverpod.dart';
import 'package:strata_admin/src/l10n.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_ui/strata_ui.dart' hide SyncPill;

/// Admin → Users (§11 screen 14, SCREEN_SPEC AdminUsersCompact /
/// AdminUsersExpanded): the pending approvals queue, then every account
/// with role and status, including a scheduled deletion and whether the
/// export was downloaded. Online and admins only: the core's
/// [Availability] decides what is shown.
///
/// The per-account intents (approve, reject, disable, enable, reset
/// password, schedule / cancel deletion, role) are not in the core API yet
/// (docs/CORE_GAPS.md); their controls are shown disabled.
class AdminUsersScreen extends ConsumerWidget {
  /// Creates the screen. [embedded] drops the compact app bar (the screen
  /// is a pane of Settings on medium and expanded).
  const new({super.key, this.embedded = false});

  /// The icon that represents this feature.
  static const IconData icon = Icons.admin_panel_settings_outlined;

  /// Rendered inside another screen's pane.
  final bool embedded;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.adminL10n;
    final users = ref.watch(adminUsersProvider);
    final compact = SizeClass.of(context) == SizeClass.compact;
    final body = switch (users) {
      AsyncData(:final value) => switch (value.availability) {
        Availability.available => AdminUsersContent(view: value),
        Availability.offline => StrataEmptyState(
          icon: Icons.cloud_off_outlined,
          title: l10n.offlineTitle,
          message: l10n.offlineBody,
          action: StrataAction(
            label: l10n.retry,
            icon: Icons.refresh,
            onPressed: () => ref.invalidate(adminUsersProvider),
          ),
        ),
        Availability.notYetAvailable => StrataEmptyState(
          icon: Icons.hourglass_empty,
          title: l10n.notYetTitle,
          message: l10n.notYetBody,
        ),
        Availability.notAllowed => StrataEmptyState(
          icon: Icons.lock_outline,
          title: l10n.notAllowedTitle,
          message: l10n.notAllowedBody,
        ),
      },
      AsyncError(:final error) => StrataEmptyState(
        icon: Icons.error_outline,
        title: l10n.loadFailed,
        message: l10n.failure(error),
        action: StrataAction(
          label: l10n.retry,
          icon: Icons.refresh,
          onPressed: () => ref.invalidate(adminUsersProvider),
        ),
      ),
      _ => Center(
        child: Semantics(
          label: l10n.loading,
          child: const CircularProgressIndicator(),
        ),
      ),
    };
    if (embedded || !compact) return Material(child: body);
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.usersTitle),
        actions: [
          _Unavailable(
            child: IconButton(
              tooltip: l10n.createAccount,
              onPressed: null,
              icon: const Icon(Icons.person_add_alt_outlined),
            ),
          ),
        ],
      ),
      body: body,
    );
  }
}

/// Wraps a control whose intent the core does not offer yet.
class _Unavailable extends StatelessWidget {
  const new({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) =>
      Tooltip(message: context.adminL10n.notAvailableYet, child: child);
}

/// The users content for one [AdminUsersView] (no providers).
class AdminUsersContent extends StatelessWidget {
  /// Creates the content.
  const new({required this.view, super.key});

  /// The core's view.
  final AdminUsersView view;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final compact = SizeClass.of(context) == SizeClass.compact;
    final pad = compact ? StrataSpacing.s4 : StrataSpacing.s8;
    return ListView(
      padding: EdgeInsets.fromLTRB(pad, StrataSpacing.s4, pad, pad),
      children: [
        if (!compact) ...[
          Text(
            l10n.breadcrumb,
            style: text.caption.copyWith(color: colors.text2),
          ),
          Row(
            children: [
              Expanded(
                child: Semantics(
                  header: true,
                  container: true,
                  child: Text(l10n.usersTitle, style: text.display),
                ),
              ),
              _Unavailable(
                child: FilledButton.icon(
                  onPressed: null,
                  icon: const Icon(Icons.person_add_alt_outlined, size: 18),
                  label: Text(l10n.createAccount),
                ),
              ),
            ],
          ),
          const SizedBox(height: StrataSpacing.s2),
        ],
        Row(
          children: [
            Icon(Icons.shield_outlined, size: 18, color: colors.text2),
            const SizedBox(width: StrataSpacing.s2),
            Expanded(
              child: Text(
                l10n.intro,
                style: text.bodySmall.copyWith(color: colors.text2),
              ),
            ),
          ],
        ),
        const SizedBox(height: StrataSpacing.s5),
        StrataSectionHeader(
          title: l10n.pendingTitle,
          count: view.pending.length,
          padding: EdgeInsets.zero,
        ),
        const SizedBox(height: StrataSpacing.s2),
        if (view.pending.isEmpty)
          Text(
            l10n.pendingEmpty,
            style: text.bodySmall.copyWith(color: colors.text2),
          )
        else if (compact)
          for (final user in view.pending) ...[
            PendingUserCard(user: user),
            const SizedBox(height: StrataSpacing.s3),
          ]
        else
          Wrap(
            spacing: StrataSpacing.s3,
            runSpacing: StrataSpacing.s3,
            children: [
              for (final user in view.pending)
                SizedBox(width: 420, child: PendingUserCard(user: user)),
            ],
          ),
        const SizedBox(height: StrataSpacing.s6),
        StrataSectionHeader(
          title: l10n.allUsers,
          count: view.users.length,
          padding: EdgeInsets.zero,
        ),
        const SizedBox(height: StrataSpacing.s2),
        if (compact)
          for (final user in view.users) UserTile(user: user)
        else
          UsersTable(users: view.users),
        const SizedBox(height: StrataSpacing.s4),
        Text(
          l10n.footnoteDisable,
          style: text.caption.copyWith(color: colors.text2),
        ),
        Text(
          l10n.footnoteReset,
          style: text.caption.copyWith(color: colors.text2),
        ),
      ],
    );
  }
}

String _date(BuildContext context, DateTime at) =>
    MaterialLocalizations.of(context).formatMediumDate(at.toLocal());

/// An account awaiting approval, with Reject / Approve.
class PendingUserCard extends StatelessWidget {
  /// Creates the card.
  const new({required this.user, super.key});

  /// The account.
  final AdminUserItem user;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return Container(
      padding: const EdgeInsets.all(StrataSpacing.s4),
      decoration: BoxDecoration(
        color: colors.surface,
        borderRadius: StrataRadii.cardRadius,
        border: Border.all(color: colors.border),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          MergeSemantics(
            child: Row(
              children: [
                const _Avatar(),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(user.displayName, style: text.bodyStrong),
                      Text(
                        l10n.requested(
                          username: user.username,
                          date: _date(context, user.created),
                        ),
                        style: text.caption.copyWith(color: colors.text2),
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
          const SizedBox(height: StrataSpacing.s3),
          Row(
            children: [
              Expanded(
                child: _Unavailable(
                  child: Semantics(
                    label: l10n.rejectSemantics(name: user.displayName),
                    excludeSemantics: true,
                    button: true,
                    enabled: false,
                    child: OutlinedButton(
                      onPressed: null,
                      child: Text(l10n.reject),
                    ),
                  ),
                ),
              ),
              const SizedBox(width: StrataSpacing.s2),
              Expanded(
                child: _Unavailable(
                  child: Semantics(
                    label: l10n.approveSemantics(name: user.displayName),
                    excludeSemantics: true,
                    button: true,
                    enabled: false,
                    child: FilledButton(
                      onPressed: null,
                      child: Text(l10n.approve),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _Avatar extends StatelessWidget {
  const new();

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return ExcludeSemantics(
      child: CircleAvatar(
        radius: 20,
        backgroundColor: colors.accentTint,
        foregroundColor: colors.accentText,
        child: const Icon(Icons.person_outline, size: 22),
      ),
    );
  }
}

/// An account's status pill; a scheduled deletion shows its date and the
/// export state below.
class UserStatus extends StatelessWidget {
  /// Creates the status for [user].
  const new({required this.user, super.key});

  /// The account.
  final AdminUserItem user;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final deletionAt = user.deletionAt;
    final downloaded = user.exportDownloadedAt;
    final label = switch (user.status) {
      'active' => l10n.statusActive,
      'disabled' => l10n.statusDisabled,
      'pending' => l10n.statusPending,
      'rejected' => l10n.statusRejected,
      'deletion_pending' =>
        deletionAt == null
            ? l10n.statusDeletionNoDate
            : l10n.statusDeletion(date: _date(context, deletionAt)),
      final other => l10n.statusOther(status: other),
    };
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        StatusPill(
          label: label,
          tone: statusTone(user.status),
          icon: statusIcon(user.status),
        ),
        if (user.status == 'deletion_pending')
          Text(
            downloaded == null
                ? l10n.exportNotDownloaded
                : l10n.exportDownloaded(date: _date(context, downloaded)),
            style: text.caption.copyWith(color: colors.text2),
          ),
      ],
    );
  }
}

/// The actions of one account (all pending core intents).
class UserActions extends StatelessWidget {
  /// Creates the actions for [user].
  const new({required this.user, super.key});

  /// The account.
  final AdminUserItem user;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final name = user.displayName;
    Widget icon(IconData data, String label) => _Unavailable(
      child: IconButton(
        onPressed: null,
        icon: Icon(data, size: 20),
        tooltip: label,
      ),
    );
    return Row(
      mainAxisSize: MainAxisSize.min,
      children: switch (user.status) {
        'deletion_pending' => [
          _Unavailable(
            child: Semantics(
              label: l10n.cancelDeletionSemantics(name: name),
              excludeSemantics: true,
              button: true,
              enabled: false,
              child: OutlinedButton(
                onPressed: null,
                child: Text(l10n.cancelDeletion),
              ),
            ),
          ),
        ],
        'disabled' => [
          _Unavailable(
            child: Semantics(
              label: l10n.enableSemantics(name: name),
              excludeSemantics: true,
              button: true,
              enabled: false,
              child: OutlinedButton(onPressed: null, child: Text(l10n.enable)),
            ),
          ),
          icon(Icons.delete_outline, l10n.deleteSemantics(name: name)),
        ],
        _ => [
          icon(Icons.block, l10n.disableSemantics(name: name)),
          icon(Icons.key_outlined, l10n.resetSemantics(name: name)),
          icon(Icons.delete_outline, l10n.deleteSemantics(name: name)),
        ],
      },
    );
  }
}

/// A user row on compact; tapping opens the account's actions.
class UserTile extends StatelessWidget {
  /// Creates the row.
  const new({required this.user, super.key});

  /// The account.
  final AdminUserItem user;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return ListTile(
      contentPadding: EdgeInsets.zero,
      minTileHeight: 64,
      leading: const _Avatar(),
      title: Text(user.displayName, style: text.bodyStrong),
      subtitle: Text(
        l10n.userSubtitle(username: user.username, role: l10n.role(user.role)),
        style: text.caption.copyWith(color: colors.text2),
      ),
      trailing: UserStatus(user: user),
      onTap: () => showModalBottomSheet<void>(
        context: context,
        showDragHandle: true,
        builder: (sheet) => SafeArea(
          child: Padding(
            padding: const EdgeInsets.fromLTRB(
              StrataSpacing.s5,
              0,
              StrataSpacing.s5,
              StrataSpacing.s5,
            ),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: [
                Semantics(
                  header: true,
                  container: true,
                  child: Text(
                    l10n.userActions(name: user.displayName),
                    style: text.titleSmall,
                  ),
                ),
                const SizedBox(height: StrataSpacing.s2),
                UserStatus(user: user),
                const SizedBox(height: StrataSpacing.s3),
                Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: UserActions(user: user),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The users table of medium and expanded layouts.
class UsersTable extends StatelessWidget {
  /// Creates the table.
  const new({required this.users, super.key});

  /// Every account but the pending ones.
  final List<AdminUserItem> users;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final head = text.caption
        .withWeight(FontWeight.w600)
        .copyWith(color: colors.text2);
    Widget cell(Widget child) => Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: StrataSpacing.s3,
        vertical: StrataSpacing.s2,
      ),
      child: child,
    );
    Widget header(String label) => cell(
      Semantics(header: true, container: true, child: Text(label, style: head)),
    );
    return DecoratedBox(
      decoration: BoxDecoration(
        color: colors.surface,
        borderRadius: StrataRadii.cardRadius,
        border: Border.all(color: colors.border),
      ),
      child: Table(
        defaultVerticalAlignment: TableCellVerticalAlignment.middle,
        columnWidths: const {
          0: FlexColumnWidth(1.6),
          1: FlexColumnWidth(1.2),
          2: FlexColumnWidth(),
          3: FlexColumnWidth(1.5),
          4: FlexColumnWidth(),
          5: IntrinsicColumnWidth(),
        },
        border: TableBorder(horizontalInside: BorderSide(color: colors.border)),
        children: [
          TableRow(
            decoration: BoxDecoration(color: colors.surface2),
            children: [
              header(l10n.columnName),
              header(l10n.columnUsername),
              header(l10n.columnRole),
              header(l10n.columnStatus),
              header(l10n.columnCreated),
              header(l10n.columnActions),
            ],
          ),
          for (final user in users)
            TableRow(
              children: [
                cell(
                  Row(
                    children: [
                      const _Avatar(),
                      const SizedBox(width: StrataSpacing.s2),
                      Flexible(
                        child: Text(user.displayName, style: text.bodyStrong),
                      ),
                    ],
                  ),
                ),
                cell(
                  Text(
                    l10n.atUsername(username: user.username),
                    textDirection: TextDirection.ltr,
                    textAlign: TextAlign.start,
                    style: text.monoSmall.copyWith(color: colors.text),
                  ),
                ),
                cell(Text(l10n.role(user.role), style: text.bodySmall)),
                cell(UserStatus(user: user)),
                cell(
                  Text(
                    _date(context, user.created),
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                ),
                cell(UserActions(user: user)),
              ],
            ),
        ],
      ),
    );
  }
}

/// "Schedule deletion of @nour?" (SCREEN_SPEC AdminUsersExpanded popover).
/// Returns `true` when confirmed. [date] is the purge date as the core
/// reports it.
class ScheduleDeletionDialog extends StatelessWidget {
  /// Creates the dialog.
  const new({required this.user, required this.date, super.key});

  /// The account.
  final AdminUserItem user;

  /// The purge date.
  final DateTime date;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    return AlertDialog(
      icon: Icon(Icons.auto_delete_outlined, color: colors.dangerText),
      title: Text(l10n.scheduleTitle(username: user.username)),
      content: Text(
        l10n.scheduleBody(
          name: user.displayName,
          date: MaterialLocalizations.of(context)
              .formatFullDate(date.toLocal()),
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: colors.dangerTint,
            foregroundColor: colors.dangerText,
          ),
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(l10n.scheduleDeletion),
        ),
      ],
    );
  }
}

/// Shows a reset password's one-time password once (§11 screen 14), with
/// copy.
class OneTimePasswordDialog extends StatelessWidget {
  /// Creates the dialog.
  const new({required this.user, required this.password, super.key});

  /// The account.
  final AdminUserItem user;

  /// The one-time password from the core.
  final String password;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    return AlertDialog(
      icon: Icon(Icons.key_outlined, color: colors.accentText),
      title: Text(l10n.oneTimeTitle(username: user.username)),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(l10n.oneTimeBody(name: user.displayName)),
          const SizedBox(height: StrataSpacing.s4),
          Container(
            padding: const EdgeInsets.all(StrataSpacing.s3),
            decoration: BoxDecoration(
              color: colors.surface2,
              borderRadius: StrataRadii.inputRadius,
            ),
            child: SelectableText(
              password,
              textAlign: TextAlign.center,
              textDirection: TextDirection.ltr,
              style: text.mono.copyWith(fontSize: StrataTypeScale.titleSmall),
            ),
          ),
        ],
      ),
      actions: [
        TextButton.icon(
          onPressed: () async {
            final messenger = ScaffoldMessenger.maybeOf(context);
            await Clipboard.setData(ClipboardData(text: password));
            messenger?.showSnackBar(SnackBar(content: Text(l10n.copied)));
          },
          icon: const Icon(Icons.copy, size: 18),
          label: Text(l10n.copy),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.done),
        ),
      ],
    );
  }
}
