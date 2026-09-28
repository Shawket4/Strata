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
    String? exportDownloadedLabel,
    String initials = '',
    String created = '12 Sep',
    String? deletionLabel,
    bool isSelf = false,
    bool passwordChangeRequired = false,
  }) => AdminUserItem(
    id: id,
    username: username,
    displayName: name,
    role: role,
    status: status,
    created: DateTime.utc(2026, 9, 12, 9),
    deletionAt: deletionAt,
    exportDownloadedAt: exportDownloadedAt,
    exportDownloadedLabel: exportDownloadedLabel,
    initials: initials,
    isSelf: isSelf,
    createdLabel: created,
    deletionLabel: deletionLabel,
    passwordChangeRequired: passwordChangeRequired,
  );

  /// Two pending approvals and five accounts.
  static final AdminUsersView view = AdminUsersView(
    availability: Availability.available,
    pending: [
      _user(
        'u-sara',
        'sara.n',
        'Sara Nabil',
        status: 'pending',
        initials: 'SN',
        created: '2h ago',
      ),
      _user(
        'u-youssef',
        'youssef.k',
        'Youssef Kamal',
        status: 'pending',
        initials: 'YK',
        created: '5h ago',
      ),
    ],
    users: [
      _user(
        'u-shawket',
        'shawket',
        'Shawket',
        role: 'admin',
        initials: 'S',
        created: '4 Jan',
        isSelf: true,
      ),
      _user(
        'u-ahmed',
        'ahmed.s',
        'Ahmed Samir',
        initials: 'AS',
        passwordChangeRequired: true,
      ),
      _user(
        'u-karim',
        'karim',
        'Karim Adel',
        status: 'deletion_pending',
        deletionAt: DateTime.utc(2026, 10, 11, 9),
        deletionLabel: '11 Oct',
        initials: 'KA',
      ),
      _user(
        'u-mona',
        'mona.h',
        'Mona Hassan',
        status: 'disabled',
        initials: 'MH',
      ),
      _user(
        'u-nour',
        'nour',
        'Nour Ali',
        status: 'deletion_pending',
        deletionAt: DateTime.utc(2026, 10, 11, 9),
        exportDownloadedAt: DateTime.utc(2026, 9, 26, 9),
        exportDownloadedLabel: 'Export downloaded Sat 12:00',
        deletionLabel: '11 Oct',
        initials: 'NA',
      ),
    ],
    query: '',
    deletionPreviewLabel: 'Deleted on 11 Oct 2026',
  );

  /// No pending approvals.
  static final AdminUsersView noPending = AdminUsersView(
    availability: Availability.available,
    pending: const [],
    users: view.users,
    query: '',
  );

  /// The list for a search that matches nobody.
  static const AdminUsersView noMatch = AdminUsersView(
    availability: Availability.available,
    pending: [],
    users: [],
    query: 'zed',
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
