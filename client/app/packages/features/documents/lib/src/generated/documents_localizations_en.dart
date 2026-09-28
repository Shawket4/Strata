// ignore: unused_import
import 'package:intl/intl.dart' as intl;

import 'documents_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class DocumentsLocalizationsEn extends DocumentsLocalizations {
  DocumentsLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get documents => 'Documents';

  @override
  String get places => 'Places';

  @override
  String get backToDocuments => 'Back to Directory';

  @override
  String get backToPlaces => 'Back to Places';

  @override
  String get moreActions => 'More actions';

  @override
  String get showContext => 'Show context';

  @override
  String get contextLabel => 'Context';

  @override
  String get breadcrumb => 'Breadcrumb';

  @override
  String get locationLabel => 'Location';

  @override
  String get notFoundTitle => 'This page isn\'t in the vault any more';

  @override
  String get notFoundMessage =>
      'It may have been deleted or merged on another device.';

  @override
  String get loading => 'Loading…';

  @override
  String get errorTitle => 'This couldn\'t be loaded';

  @override
  String errorMessage({required String code}) {
    return 'The app\'s local data returned an error ($code).';
  }

  @override
  String get pendingSync => 'Not synced yet';

  @override
  String get documentKind => 'Document';

  @override
  String get placeKind => 'Place';

  @override
  String docSubtitle({required String type, required String copy}) {
    return '$type · $copy';
  }

  @override
  String get expiresLabel => 'Expires';

  @override
  String get renewalLabel => 'Renewal';

  @override
  String get reminderLabel => 'Reminder';

  @override
  String get typeLabel => 'Type';

  @override
  String get concernsLabel => 'Concerns';

  @override
  String get copiesLabel => 'Copies';

  @override
  String copiesCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count copies',
      one: '1 copy',
      zero: 'No copies',
    );
    return '$_temp0';
  }

  @override
  String get whereItIs => 'Where it is';

  @override
  String get locationUnknown => 'Location unknown';

  @override
  String withHolder({required String name}) {
    return 'With $name';
  }

  @override
  String get nobodyHasIt => 'Nobody has it now';

  @override
  String get lastWith => '· last with';

  @override
  String get withLabel => 'With';

  @override
  String get recordMove => 'Record a move';

  @override
  String get custodyHistory => 'Custody history';

  @override
  String get custodyHint =>
      'Location and holder change only through these events';

  @override
  String custodyCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count events · newest first',
      one: '1 event · newest first',
      zero: 'No events',
    );
    return '$_temp0';
  }

  @override
  String get noCustody => 'No custody events recorded yet.';

  @override
  String get custodyStoredAt => 'Stored at';

  @override
  String get custodyMovedTo => 'Moved to';

  @override
  String get custodyHandedTo => 'Handed to';

  @override
  String get custodyReturnedBy => 'Returned by';

  @override
  String get custodySentTo => 'Sent to';

  @override
  String get custodyReceivedFrom => 'Received from';

  @override
  String get custodyLost => 'Lost';

  @override
  String get custodyFound => 'Found';

  @override
  String get custodyDestroyed => 'Destroyed';

  @override
  String get statusStored => 'Stored';

  @override
  String get statusCheckedOut => 'Checked out';

  @override
  String get statusThirdParty => 'Third party';

  @override
  String get statusLost => 'Lost';

  @override
  String get statusDestroyed => 'Destroyed';

  @override
  String get typeContract => 'Contract';

  @override
  String get typeId => 'ID';

  @override
  String get typeLicence => 'Licence';

  @override
  String get typeDeed => 'Deed';

  @override
  String get typeInvoice => 'Invoice';

  @override
  String get typeCertificate => 'Certificate';

  @override
  String get typeOther => 'Other';

  @override
  String get copyOriginal => 'Original';

  @override
  String get copyCertified => 'Certified copy';

  @override
  String get copyCopy => 'Copy';

  @override
  String get copyDigital => 'Digital';

  @override
  String get yourNotes => 'Your notes';

  @override
  String get yourNotesHint => 'Only you edit this';

  @override
  String get openNote => 'Open note';

  @override
  String get undoAi => 'Undo';

  @override
  String get graph => 'Graph';

  @override
  String get placesTree => 'Places';

  @override
  String placesTreeLabel({required String place}) {
    return '$place and places inside it';
  }

  @override
  String get noSubPlaces => 'No places inside';

  @override
  String get topLevelPlace => 'Top level';

  @override
  String partOf({required String place}) {
    return 'part of $place';
  }

  @override
  String get everythingHere => 'Everything here';

  @override
  String everythingHereCaption({required String place}) {
    return 'Documents at $place, including places inside it';
  }

  @override
  String documentsCount({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count documents',
      one: '1 document',
      zero: 'No documents',
    );
    return '$_temp0';
  }

  @override
  String get noDocumentsHere => 'Nothing is stored here yet.';

  @override
  String get colDocument => 'Document';

  @override
  String get colWhere => 'Where';

  @override
  String get colHolder => 'Holder';

  @override
  String get colStatus => 'Status';

  @override
  String get recentMovements => 'Recent movements';

  @override
  String get noMovements => 'No movements recorded here yet.';

  @override
  String get aliases => 'Aliases';

  @override
  String recordMoveSubtitle({required String document, required String place}) {
    return '$document · now in $place';
  }

  @override
  String recordMoveSubtitleUnknown({required String document}) {
    return '$document';
  }

  @override
  String get whatHappened => 'What happened';

  @override
  String get eventMovedTo => 'Moved to';

  @override
  String get eventHandedTo => 'Handed to';

  @override
  String get eventReturned => 'Returned';

  @override
  String get eventSentTo => 'Sent to third party';

  @override
  String get eventLost => 'Lost';

  @override
  String get toPlace => 'To place';

  @override
  String get holderAfter => 'Holder after';

  @override
  String get nobody => 'Nobody';

  @override
  String get documentField => 'Document';

  @override
  String get dateField => 'Date';

  @override
  String get dateToday => 'Today';

  @override
  String get pickDate => 'Choose a date';

  @override
  String get recordMoveFooter => 'Adds a cited event to ## Custody';

  @override
  String get recordMoveSubmit => 'Record move';

  @override
  String get cancel => 'Cancel';

  @override
  String get close => 'Close';

  @override
  String get searchPlaces => 'Search places';

  @override
  String get searchPeople => 'Search people';

  @override
  String dateShort({required DateTime date}) {
    final intl.DateFormat dateDateFormat = intl.DateFormat(
      'EEE d MMM y',
      localeName,
    );
    final String dateString = dateDateFormat.format(date);

    return '$dateString';
  }

  @override
  String get userNotesEmpty => 'Nothing written yet.';

  @override
  String get userNotesEdit => 'Edit notes';

  @override
  String get userNotesField => 'Your notes';

  @override
  String get userNotesSave => 'Save notes';

  @override
  String get userNotesSaved => 'Notes saved';

  @override
  String userNotesFailed({required String code}) {
    return 'Couldn\'t save your notes ($code).';
  }

  @override
  String get noMentions => 'No notes mention this yet.';

  @override
  String get mentioningNotes => 'Mentioning notes';

  @override
  String get newestFirst => 'Newest first';

  @override
  String get noRenewal => 'No renewal task';

  @override
  String get expiringSoon => 'Expiring soon';

  @override
  String lastWithName({required String name}) {
    return 'Last with $name';
  }

  @override
  String aiConfidence({required String value}) {
    return 'AI · $value';
  }

  @override
  String get aiTag => 'AI';

  @override
  String atPlace({required String place}) {
    return 'at $place';
  }

  @override
  String get choosePlace => 'Choose where it went.';

  @override
  String get choosePerson => 'Choose who has it.';

  @override
  String get chooseThirdParty => 'Choose the third party.';

  @override
  String get chooseDocument => 'Choose a document.';

  @override
  String get thirdParty => 'Third party';

  @override
  String get searchCompanies => 'Search companies';

  @override
  String get currentPlace => 'Current';

  @override
  String get moveRecorded => 'Move recorded';

  @override
  String moveFailed({required String code}) {
    return 'Couldn\'t record the move ($code).';
  }

  @override
  String get outWithPeople => 'Out with people';

  @override
  String get nobodyOut => 'Everything from here is in place.';

  @override
  String placeDocuments({required int count}) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count documents',
      one: '1 document',
      zero: 'empty',
    );
    return '$_temp0';
  }
}
