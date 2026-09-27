/// Strata accounts feature (PLAN §11 screens 1 and 15, §12.7): sign in, sign
/// up, waiting for approval, the account sheet with sign-out, and the
/// restricted screens (account disabled, deletion pending, password change
/// required). UI only; view-models come from the Rust core (L15).
library;

export 'src/account_sheet.dart';
export 'src/auth_layout.dart'
    show AuthLayout, BrandPanel, FormAlert, LabeledField, PrimaryButton;
export 'src/l10n.dart';
export 'src/pending_approval_screen.dart';
export 'src/restricted_screens.dart';
export 'src/sign_in_screen.dart';
export 'src/sign_up_screen.dart';
