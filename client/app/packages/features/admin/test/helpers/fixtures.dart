import 'package:strata_state/strata_state.dart';

/// Admin → Users from SCREEN_SPEC AdminUsersExpanded.
abstract final class AdminFixtures {
  static AdminUserItem _user(
    String id,
    String username,
    String name, {
    String role = 'member',
    String status = 'active',
    DateTime? deletionAt,
    DateTime? exportDownloadedAt,
  }) => AdminUserItem(
    id: id,
    username: username,
    displayName: name,
    role: role,
    status: status,
    created: DateTime.utc(2026, 9, 12, 9),
    deletionAt: deletionAt,
    exportDownloadedAt: exportDownloadedAt,
    initials: '',
    isSelf: false,
    createdLabel: '',
    passwordChangeRequired: false,
  );

  /// Two pending approvals and five accounts.
  static final AdminUsersView view = AdminUsersView(
    availability: Availability.available,
    pending: [
      _user('u-sara', 'sara.n', 'Sara Nabil', status: 'pending'),
      _user('u-youssef', 'youssef.k', 'Youssef Kamal', status: 'pending'),
    ],
    users: [
      _user('u-shawket', 'shawket', 'Shawket', role: 'admin'),
      _user('u-ahmed', 'ahmed.s', 'Ahmed Samir'),
      _user(
        'u-karim',
        'karim',
        'Karim Adel',
        status: 'deletion_pending',
        deletionAt: DateTime.utc(2026, 10, 11, 9),
      ),
      _user('u-mona', 'mona.h', 'Mona Hassan', status: 'disabled'),
      _user(
        'u-nour',
        'nour',
        'Nour Ali',
        status: 'deletion_pending',
        deletionAt: DateTime.utc(2026, 10, 11, 9),
        exportDownloadedAt: DateTime.utc(2026, 9, 26, 9),
      ),
    ],
    query: '',
  );

  /// No pending approvals.
  static final AdminUsersView noPending = AdminUsersView(
    availability: Availability.available,
    pending: const [],
    users: view.users,
    query: '',
  );

  /// Availability only.
  static AdminUsersView unavailable(Availability availability) =>
      AdminUsersView(
        availability: availability,
        pending: const [],
        users: const [],
        query: '',
      );
}
