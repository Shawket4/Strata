import 'package:flutter/material.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_ui/strata_ui.dart';

/// How a document status (`stored`, `checked-out`, `with-third-party`,
/// `lost`, `destroyed`; free text otherwise) is shown: 1:1 label and tone.
({String label, StatusTone tone, IconData icon}) documentStatus(
  DocumentsLocalizations l10n,
  String status,
) => switch (status) {
  'stored' => (
    label: l10n.statusStored,
    tone: StatusTone.success,
    icon: Icons.inventory_2_outlined,
  ),
  'checked-out' => (
    label: l10n.statusCheckedOut,
    tone: StatusTone.warning,
    icon: Icons.outbox_outlined,
  ),
  'with-third-party' => (
    label: l10n.statusThirdParty,
    tone: StatusTone.info,
    icon: Icons.handshake_outlined,
  ),
  'lost' => (
    label: l10n.statusLost,
    tone: StatusTone.danger,
    icon: Icons.help_outline,
  ),
  'destroyed' => (
    label: l10n.statusDestroyed,
    tone: StatusTone.neutral,
    icon: Icons.delete_outline,
  ),
  _ => (label: status, tone: StatusTone.neutral, icon: Icons.circle_outlined),
};

/// The label of a `doc-type` value (free text is shown as written).
String documentTypeLabel(DocumentsLocalizations l10n, String docType) =>
    switch (docType) {
      'contract' => l10n.typeContract,
      'id' => l10n.typeId,
      'licence' => l10n.typeLicence,
      'deed' => l10n.typeDeed,
      'invoice' => l10n.typeInvoice,
      'certificate' => l10n.typeCertificate,
      'other' => l10n.typeOther,
      _ => docType,
    };

/// The label of a `copy` value (free text is shown as written).
String copyLabel(DocumentsLocalizations l10n, String copy) => switch (copy) {
  'original' => l10n.copyOriginal,
  'certified copy' => l10n.copyCertified,
  'copy' => l10n.copyCopy,
  'digital' => l10n.copyDigital,
  _ => copy,
};

/// How a custody event type (PLAN §6.12) is shown: label, icon and tone.
({String label, IconData icon, StatusTone tone}) custodyKind(
  DocumentsLocalizations l10n,
  String kind,
) => switch (kind) {
  'stored-at' => (
    label: l10n.custodyStoredAt,
    icon: Icons.inventory_2_outlined,
    tone: StatusTone.info,
  ),
  'moved-to' => (
    label: l10n.custodyMovedTo,
    icon: Icons.move_down,
    tone: StatusTone.info,
  ),
  'handed-to' => (
    label: l10n.custodyHandedTo,
    icon: Icons.front_hand_outlined,
    tone: StatusTone.warning,
  ),
  'returned-by' => (
    label: l10n.custodyReturnedBy,
    icon: Icons.keyboard_return,
    tone: StatusTone.success,
  ),
  'sent-to' => (
    label: l10n.custodySentTo,
    icon: Icons.send_outlined,
    tone: StatusTone.info,
  ),
  'received-from' => (
    label: l10n.custodyReceivedFrom,
    icon: Icons.call_received,
    tone: StatusTone.success,
  ),
  'lost' => (
    label: l10n.custodyLost,
    icon: Icons.help_outline,
    tone: StatusTone.danger,
  ),
  'found' => (
    label: l10n.custodyFound,
    icon: Icons.search,
    tone: StatusTone.success,
  ),
  'destroyed' => (
    label: l10n.custodyDestroyed,
    icon: Icons.delete_outline,
    tone: StatusTone.neutral,
  ),
  _ => (label: kind, icon: Icons.circle_outlined, tone: StatusTone.neutral),
};
