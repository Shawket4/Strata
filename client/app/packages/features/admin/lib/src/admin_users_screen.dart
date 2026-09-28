import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_hooks/flutter_hooks.dart';
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
/// Every control forwards a core intent (approve, reject, role, disable /
/// enable, reset password, schedule / cancel deletion, create account) and
/// re-reads the list; the search field asks the core for the filtered list
/// (`load_admin_users(query)`).
class AdminUsersScreen extends HookConsumerWidget {
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
    final query = useState('');
    final users = ref.watch(adminUsersProvider(query.value));
    // The last list stays on screen while the next query loads, so the
    // search field keeps its focus.
    final shown = useRef<AdminUsersView?>(null);
    if (users.value case final view?) shown.value = view;
    final compact = SizeClass.of(context) == SizeClass.compact;
    void retry() => ref.invalidate(adminUsersProvider(query.value));
    final last = shown.value;
    final body = switch (users) {
      AsyncError(:final error) when last == null => StrataEmptyState(
        icon: Icons.error_outline,
        title: l10n.loadFailed,
        message: l10n.failure(error),
        action: StrataAction(
          label: l10n.retry,
          icon: Icons.refresh,
          onPressed: retry,
        ),
      ),
      _ when last == null => Center(
        child: Semantics(
          label: l10n.loading,
          child: const CircularProgressIndicator(),
        ),
      ),
      _ => switch (last.availability) {
        Availability.available => AdminUsersContent(
          view: last,
          onQueryChanged: (value) => query.value = value,
        ),
        Availability.offline => StrataEmptyState(
          icon: Icons.cloud_off_outlined,
          title: l10n.offlineTitle,
          message: l10n.offlineBody,
          action: StrataAction(
            label: l10n.retry,
            icon: Icons.refresh,
            onPressed: retry,
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
    };
    if (embedded || !compact) return Material(child: body);
    final available = last?.availability == Availability.available;
    return Scaffold(
      appBar: AppBar(
        title: Text(l10n.usersTitle),
        actions: [
          if (available)
            IconButton(
              tooltip: l10n.createAccount,
              onPressed: () =>
                  unawaited(createAccount(context, ref, last?.query ?? '')),
              icon: const Icon(Icons.person_add_alt_outlined),
            ),
        ],
      ),
      body: body,
    );
  }
}

/// Runs an admin [intent] on the core, re-reads the list for [query] and
/// shows [done] (when given) or the failure in a snack bar.
Future<void> runAdminIntent(
  BuildContext context,
  WidgetRef ref,
  String query,
  Future<Object?> Function(CoreApi core) intent, {
  String? done,
}) async {
  final messenger = ScaffoldMessenger.maybeOf(context);
  final l10n = context.adminL10n;
  try {
    await intent(ref.read(coreApiProvider));
    ref.invalidate(adminUsersProvider(query));
    if (done != null) messenger?.showSnackBar(SnackBar(content: Text(done)));
  } on Object catch (error) {
    messenger?.showSnackBar(SnackBar(content: Text(l10n.failure(error))));
  }
}

/// "Create account": asks for the account in [CreateAccountDialog] and
/// forwards it to `create_user`.
Future<void> createAccount(
  BuildContext context,
  WidgetRef ref,
  String query,
) async {
  final l10n = context.adminL10n;
  final request = await showDialog<NewUserRequest>(
    context: context,
    builder: (_) => const CreateAccountDialog(),
  );
  if (request == null || !context.mounted) return;
  await runAdminIntent(
    context,
    ref,
    query,
    (core) => core.createUser(request: request),
    done: l10n.created(username: request.username),
  );
}

/// The users content for one [AdminUsersView]: the pending queue, the
/// search field and every account.
class AdminUsersContent extends ConsumerWidget {
  /// Creates the content.
  const new({required this.view, super.key, this.onQueryChanged});

  /// The core's view.
  final AdminUsersView view;

  /// The search text changed (the screen asks the core for that list).
  final ValueChanged<String>? onQueryChanged;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
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
              FilledButton.icon(
                onPressed: () =>
                    unawaited(createAccount(context, ref, view.query)),
                icon: const Icon(Icons.person_add_alt_outlined, size: 18),
                label: Text(l10n.createAccount),
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
            PendingUserCard(user: user, query: view.query),
            const SizedBox(height: StrataSpacing.s3),
          ]
        else
          Wrap(
            spacing: StrataSpacing.s3,
            runSpacing: StrataSpacing.s3,
            children: [
              for (final user in view.pending)
                SizedBox(
                  width: 420,
                  child: PendingUserCard(user: user, query: view.query),
                ),
            ],
          ),
        const SizedBox(height: StrataSpacing.s6),
        Row(
          children: [
            Expanded(
              child: StrataSectionHeader(
                title: l10n.allUsers,
                count: view.users.length,
                padding: EdgeInsets.zero,
              ),
            ),
            if (!compact)
              SizedBox(
                width: 280,
                child: _SearchField(
                  query: view.query,
                  onChanged: onQueryChanged,
                ),
              ),
          ],
        ),
        if (compact) ...[
          const SizedBox(height: StrataSpacing.s2),
          _SearchField(query: view.query, onChanged: onQueryChanged),
        ],
        const SizedBox(height: StrataSpacing.s2),
        if (view.users.isEmpty && view.query.isNotEmpty)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s3),
            child: Text(
              l10n.noMatches(query: view.query),
              style: text.bodySmall.copyWith(color: colors.text2),
            ),
          )
        else if (compact)
          for (final user in view.users) UserTile(user: user, query: view.query)
        else
          UsersTable(users: view.users, query: view.query),
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

class _SearchField extends HookWidget {
  const new({required this.query, required this.onChanged});

  final String query;
  final ValueChanged<String>? onChanged;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final controller = useTextEditingController(text: query);
    return TextField(
      controller: controller,
      onChanged: onChanged,
      textInputAction: TextInputAction.search,
      decoration: InputDecoration(
        isDense: true,
        prefixIcon: const Icon(Icons.search, size: 20),
        hintText: l10n.searchLabel,
        labelText: l10n.searchLabel,
        floatingLabelBehavior: FloatingLabelBehavior.never,
      ),
    );
  }
}

/// An account awaiting approval, with Reject / Approve.
class PendingUserCard extends ConsumerWidget {
  /// Creates the card.
  const new({required this.user, super.key, this.query = ''});

  /// The account.
  final AdminUserItem user;

  /// The list's query (re-read after an answer).
  final String query;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final name = user.displayName;
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
                StrataAvatar(initials: user.initials),
                const SizedBox(width: StrataSpacing.s3),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(name, style: text.bodyStrong),
                      Text(
                        l10n.requested(
                          username: user.username,
                          date: user.createdLabel,
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
                child: Semantics(
                  label: l10n.rejectSemantics(name: name),
                  excludeSemantics: true,
                  button: true,
                  child: OutlinedButton(
                    onPressed: () => unawaited(
                      runAdminIntent(
                        context,
                        ref,
                        query,
                        (core) => core.rejectUser(id: user.id),
                        done: l10n.rejected(name: name),
                      ),
                    ),
                    child: Text(l10n.reject),
                  ),
                ),
              ),
              const SizedBox(width: StrataSpacing.s2),
              Expanded(
                child: Semantics(
                  label: l10n.approveSemantics(name: name),
                  excludeSemantics: true,
                  button: true,
                  child: FilledButton(
                    onPressed: () => unawaited(
                      runAdminIntent(
                        context,
                        ref,
                        query,
                        (core) => core.approveUser(id: user.id),
                        done: l10n.approved(name: name),
                      ),
                    ),
                    child: Text(l10n.approve),
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

/// The name of an account with the "you" badge on the admin's own row.
class _UserName extends StatelessWidget {
  const new({required this.user});

  final AdminUserItem user;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final text = context.strataText;
    return Wrap(
      spacing: StrataSpacing.s2,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        Text(user.displayName, style: text.bodyStrong),
        if (user.isSelf) StatusPill(label: l10n.you, tone: StatusTone.info),
      ],
    );
  }
}

/// An account's status pill; a scheduled deletion shows the export state
/// below, a reset password that the user must replace says so.
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
    final downloaded = user.exportDownloadedAt;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        _StatusPill(user: user),
        if (user.status == 'deletion_pending') ...[
          const SizedBox(height: StrataSpacing.s1),
          Text(
            downloaded == null
                ? l10n.exportNotDownloaded
                : l10n.exportDownloaded(date: _date(context, downloaded)),
            style: text.caption.copyWith(color: colors.text2),
          ),
        ],
        if (user.passwordChangeRequired) ...[
          const SizedBox(height: StrataSpacing.s1),
          Text(
            l10n.passwordChangePending,
            style: text.caption.copyWith(color: colors.text2),
          ),
        ],
      ],
    );
  }
}

/// The export download time has no core label yet (docs/CORE_GAPS.md, Still
/// open): the instant is formatted by the platform's localisations.
String _date(BuildContext context, DateTime at) =>
    MaterialLocalizations.of(context).formatMediumDate(at.toLocal());

/// The role of an account: a picker for other accounts, the role's name on
/// the admin's own row.
class UserRole extends ConsumerWidget {
  /// Creates the role cell for [user].
  const new({required this.user, super.key, this.query = ''});

  /// The account.
  final AdminUserItem user;

  /// The list's query (re-read after a change).
  final String query;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.adminL10n;
    final text = context.strataText;
    if (user.isSelf || user.status == 'pending') {
      return Text(l10n.role(user.role), style: text.bodySmall);
    }
    return Semantics(
      label: l10n.roleSemantics(name: user.displayName),
      container: true,
      child: DropdownButtonHideUnderline(
        child: DropdownButton<String>(
          value: user.role,
          isExpanded: true,
          style: text.bodySmall.copyWith(color: context.strataColors.text),
          items: [
            for (final role in const ['admin', 'member'])
              DropdownMenuItem(
                value: role,
                child: Text(
                  l10n.role(role),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                ),
              ),
          ],
          onChanged: (role) {
            if (role == null || role == user.role) return;
            unawaited(
              runAdminIntent(
                context,
                ref,
                query,
                (core) => core.setUserRole(id: user.id, role: role),
              ),
            );
          },
        ),
      ),
    );
  }
}

/// The actions of one account, by its status: disable / reset / delete for
/// an active account, enable / delete for a disabled one, cancel for a
/// scheduled deletion, nothing for the admin's own row.
class UserActions extends ConsumerWidget {
  /// Creates the actions for [user].
  const new({required this.user, super.key, this.query = ''});

  /// The account.
  final AdminUserItem user;

  /// The list's query (re-read after an action).
  final String query;

  Future<void> _reset(BuildContext context, WidgetRef ref) async {
    final messenger = ScaffoldMessenger.maybeOf(context);
    final l10n = context.adminL10n;
    try {
      final password = await ref
          .read(coreApiProvider)
          .resetPassword(id: user.id);
      ref.invalidate(adminUsersProvider(query));
      if (!context.mounted) return;
      await showDialog<void>(
        context: context,
        builder: (_) => OneTimePasswordDialog(user: user, password: password),
      );
    } on Object catch (error) {
      messenger?.showSnackBar(SnackBar(content: Text(l10n.failure(error))));
    }
  }

  Future<void> _delete(BuildContext context, WidgetRef ref) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (_) => ScheduleDeletionDialog(user: user),
    );
    if (confirmed != true || !context.mounted) return;
    await runAdminIntent(
      context,
      ref,
      query,
      (core) => core.scheduleDeletion(id: user.id),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final name = user.displayName;
    if (user.isSelf) {
      return Text(
        l10n.yourAccount,
        style: text.caption.copyWith(color: colors.text2),
      );
    }
    void run(Future<Object?> Function(CoreApi core) intent) =>
        unawaited(runAdminIntent(context, ref, query, intent));
    Widget icon(IconData data, String label, VoidCallback onPressed) =>
        IconButton(
          onPressed: onPressed,
          icon: Icon(data, size: 20),
          tooltip: label,
        );
    final delete = icon(
      Icons.delete_outline,
      l10n.deleteSemantics(name: name),
      () => unawaited(_delete(context, ref)),
    );
    return Wrap(
      crossAxisAlignment: WrapCrossAlignment.center,
      children: switch (user.status) {
        'deletion_pending' => [
          Semantics(
            label: l10n.cancelDeletionSemantics(name: name),
            excludeSemantics: true,
            button: true,
            child: OutlinedButton(
              onPressed: () => run((core) => core.cancelDeletion(id: user.id)),
              child: Text(l10n.cancelDeletion),
            ),
          ),
        ],
        'disabled' => [
          Semantics(
            label: l10n.enableSemantics(name: name),
            excludeSemantics: true,
            button: true,
            child: OutlinedButton(
              onPressed: () => run(
                (core) => core.setUserEnabled(id: user.id, enabled: true),
              ),
              child: Text(l10n.enable),
            ),
          ),
          delete,
        ],
        'pending' || 'rejected' => const [],
        _ => [
          icon(
            Icons.block,
            l10n.disableSemantics(name: name),
            () =>
                run((core) => core.setUserEnabled(id: user.id, enabled: false)),
          ),
          icon(
            Icons.key_outlined,
            l10n.resetSemantics(name: name),
            () => unawaited(_reset(context, ref)),
          ),
          delete,
        ],
      },
    );
  }
}

/// A user row on compact; tapping opens the account's actions.
class UserTile extends ConsumerWidget {
  /// Creates the row.
  const new({required this.user, super.key, this.query = ''});

  /// The account.
  final AdminUserItem user;

  /// The list's query.
  final String query;

  void _open(BuildContext context, WidgetRef ref) {
    final l10n = context.adminL10n;
    final text = context.strataText;
    unawaited(
      showModalBottomSheet<void>(
        context: context,
        showDragHandle: true,
        isScrollControlled: true,
        builder: (sheet) => SafeArea(
          child: SingleChildScrollView(
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
                  child: Text(
                    l10n.userActions(name: user.displayName),
                    style: text.titleSmall,
                  ),
                ),
                const SizedBox(height: StrataSpacing.s2),
                UserStatus(user: user),
                if (!user.isSelf && user.status == 'active') ...[
                  const SizedBox(height: StrataSpacing.s3),
                  Align(
                    alignment: AlignmentDirectional.centerStart,
                    child: OutlinedButton(
                      onPressed: () {
                        Navigator.of(sheet).pop();
                        unawaited(
                          runAdminIntent(
                            context,
                            ref,
                            query,
                            (core) => core.setUserRole(
                              id: user.id,
                              role: user.role == 'admin' ? 'member' : 'admin',
                            ),
                          ),
                        );
                      },
                      child: Text(
                        user.role == 'admin' ? l10n.makeMember : l10n.makeAdmin,
                      ),
                    ),
                  ),
                ],
                const SizedBox(height: StrataSpacing.s3),
                Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: UserActions(user: user, query: query),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final deletion = user.status == 'deletion_pending';
    final downloaded = user.exportDownloadedAt;
    return InkWell(
      onTap: () => _open(context, ref),
      child: ConstrainedBox(
        constraints: const BoxConstraints(minHeight: 64),
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: StrataSpacing.s2),
          child: Row(
            children: [
              StrataAvatar(initials: user.initials),
              const SizedBox(width: StrataSpacing.s3),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _UserName(user: user),
                    Text(
                      deletion
                          ? (downloaded == null
                                ? l10n.exportNotDownloaded
                                : l10n.exportDownloaded(
                                    date: _date(context, downloaded),
                                  ))
                          : l10n.userSubtitle(
                              username: user.username,
                              role: l10n.role(user.role),
                            ),
                      style: text.caption.copyWith(color: colors.text2),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: StrataSpacing.s2),
              Flexible(child: _StatusPill(user: user)),
            ],
          ),
        ),
      ),
    );
  }
}

class _StatusPill extends StatelessWidget {
  const new({required this.user});

  final AdminUserItem user;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final deletion = user.deletionLabel;
    final label = switch (user.status) {
      'active' => l10n.statusActive,
      'disabled' => l10n.statusDisabled,
      'pending' => l10n.statusPending,
      'rejected' => l10n.statusRejected,
      'deletion_pending' =>
        deletion == null
            ? l10n.statusDeletionNoDate
            : l10n.statusDeletion(date: deletion),
      final other => l10n.statusOther(status: other),
    };
    return StatusPill(
      label: label,
      tone: statusTone(user.status),
      icon: statusIcon(user.status),
    );
  }
}

/// The users table of medium and expanded layouts.
class UsersTable extends StatelessWidget {
  /// Creates the table.
  const new({required this.users, super.key, this.query = ''});

  /// Every account but the pending ones.
  final List<AdminUserItem> users;

  /// The list's query.
  final String query;

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
          5: FlexColumnWidth(1.4),
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
                      StrataAvatar(initials: user.initials, size: 32),
                      const SizedBox(width: StrataSpacing.s2),
                      Flexible(child: _UserName(user: user)),
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
                cell(UserRole(user: user, query: query)),
                cell(UserStatus(user: user)),
                cell(
                  Text(
                    user.createdLabel,
                    style: text.bodySmall.copyWith(color: colors.text2),
                  ),
                ),
                cell(UserActions(user: user, query: query)),
              ],
            ),
        ],
      ),
    );
  }
}

/// "Schedule deletion of @nour?" (SCREEN_SPEC AdminUsersExpanded popover).
/// Returns `true` when confirmed. The purge date is known only once the
/// deletion is scheduled (the row's status shows it then).
class ScheduleDeletionDialog extends StatelessWidget {
  /// Creates the dialog.
  const new({required this.user, super.key});

  /// The account.
  final AdminUserItem user;

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final colors = context.strataColors;
    return AlertDialog(
      icon: Icon(Icons.auto_delete_outlined, color: colors.dangerText),
      title: Text(l10n.scheduleTitle(username: user.username)),
      content: Text(l10n.scheduleBody(name: user.displayName)),
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

/// "Create account": username, display name, a temporary password and the
/// role. Pops the [NewUserRequest] (the core validates it).
class CreateAccountDialog extends HookWidget {
  /// Creates the dialog.
  const new({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.adminL10n;
    final username = useTextEditingController();
    final name = useTextEditingController();
    final password = useTextEditingController();
    final role = useState('member');
    return AlertDialog(
      title: Text(l10n.createTitle),
      scrollable: true,
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Text(l10n.createBody),
          const SizedBox(height: StrataSpacing.s4),
          TextField(
            controller: username,
            autofocus: true,
            textDirection: TextDirection.ltr,
            decoration: InputDecoration(labelText: l10n.fieldUsername),
          ),
          const SizedBox(height: StrataSpacing.s3),
          TextField(
            controller: name,
            decoration: InputDecoration(labelText: l10n.fieldDisplayName),
          ),
          const SizedBox(height: StrataSpacing.s3),
          TextField(
            controller: password,
            obscureText: true,
            textDirection: TextDirection.ltr,
            decoration: InputDecoration(labelText: l10n.fieldPassword),
          ),
          const SizedBox(height: StrataSpacing.s3),
          DropdownButtonFormField<String>(
            initialValue: role.value,
            decoration: InputDecoration(labelText: l10n.fieldRole),
            items: [
              for (final r in const ['member', 'admin'])
                DropdownMenuItem(value: r, child: Text(l10n.role(r))),
            ],
            onChanged: (value) => role.value = value ?? role.value,
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(
            NewUserRequest(
              username: username.text,
              displayName: name.text,
              password: password.text,
              role: role.value,
            ),
          ),
          child: Text(l10n.create),
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
