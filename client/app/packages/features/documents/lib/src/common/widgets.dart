import 'package:flutter/material.dart';
import 'package:strata_documents/src/common/l10n.dart';
import 'package:strata_state/strata_state.dart' hide RelationChip;
import 'package:strata_ui/strata_ui.dart';

/// Opens a note, optionally at a block (`anchor` without `^`) or heading.
typedef OpenNoteAt = void Function(String noteId, String? anchor);

/// Navigation the host app provides to entity-style pages (documents,
/// places, people, companies). Every callback is optional: without it the
/// matching links render as plain text.
@immutable
class EntityLinks {
  /// Creates the links.
  const new({
    this.onOpenEntity,
    this.onOpenNote,
    this.onOpenMindMap,
    this.onBack,
  });

  /// The links of the closest [EntityLinksScope] (none when absent).
  factory of(BuildContext context) =>
      context.dependOnInheritedWidgetOfExactType<EntityLinksScope>()?.links ??
      const EntityLinks();

  /// Opens a person, company, document or place page (its ID).
  final ValueChanged<String>? onOpenEntity;

  /// Opens a note at a block.
  final OpenNoteAt? onOpenNote;

  /// Opens a local mind map (its ID).
  final ValueChanged<String>? onOpenMindMap;

  /// Leaves a full-screen page (compact).
  final VoidCallback? onBack;

}

/// Provides [EntityLinks] to a page's widgets.
class EntityLinksScope extends InheritedWidget {
  /// Provides [links] to [child].
  const new({required this.links, required super.child, super.key});

  /// The links.
  final EntityLinks links;

  @override
  bool updateShouldNotify(EntityLinksScope oldWidget) =>
      oldWidget.links != links;
}

/// A reference to a person, company, document or place: a link when it
/// resolves locally and the host can open it, plain text otherwise.
class EntityLink extends StatelessWidget {
  /// Creates the link for [entity].
  const new(this.entity, {super.key, this.style});

  /// The reference.
  final EntityRef entity;

  /// Text style (defaults to body small).
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final base = style ?? context.strataText.bodySmall;
    final id = entity.id;
    final open = EntityLinks.of(context).onOpenEntity;
    if (id == null || open == null) {
      return Text(entity.title, style: base);
    }
    return StrataTapTarget(
      semanticLabel: entity.title,
      onTap: () => open(id),
      child: Text(
        entity.title,
        style: base
            .withWeight(FontWeight.w600)
            .copyWith(color: colors.accentText),
      ),
    );
  }
}

/// Citation chips of an AI bullet or custody event; each opens the cited
/// block.
class CitationRow extends StatelessWidget {
  /// Creates the row for [citations].
  const new(this.citations, {super.key, this.trailing = const []});

  /// The citations.
  final List<Citation> citations;

  /// Extra widgets after the chips (AI tag, Undo).
  final List<Widget> trailing;

  @override
  Widget build(BuildContext context) {
    final open = EntityLinks.of(context).onOpenNote;
    return Wrap(
      spacing: StrataSpacing.s1,
      runSpacing: StrataSpacing.s1,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        for (final citation in citations)
          CitationChip(
            label: citation.target,
            blockRef: citation.anchor,
            onPressed: citation.noteId == null || open == null
                ? null
                : () => open(citation.noteId!, citation.anchor),
          ),
        ...trailing,
      ],
    );
  }
}

/// A node-kind glyph in a tinted square (page avatars and list leading).
class KindAvatar extends StatelessWidget {
  /// Creates the avatar.
  const new({required this.kind, super.key, this.size = 40});

  /// The kind.
  final NodeKind kind;

  /// Edge length.
  final double size;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: kind == NodeKind.place ? colors.accentTint : colors.surface2,
        borderRadius: BorderRadius.circular(size * 0.25),
      ),
      child: NodeKindGlyph(kind: kind, size: size * 0.55, decorative: true),
    );
  }
}

/// A section heading with an optional caption and trailing action.
class PageSection extends StatelessWidget {
  /// Creates the section.
  const new({
    required this.title,
    required this.children,
    super.key,
    this.caption,
    this.trailing,
  });

  /// Heading.
  final String title;

  /// Caption after the heading (e.g. "3 events · newest first").
  final String? caption;

  /// Trailing action.
  final Widget? trailing;

  /// Content.
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final colors = context.strataColors;
    final text = context.strataText;
    final note = caption;
    final end = trailing;
    return Padding(
      padding: const EdgeInsets.only(top: StrataSpacing.s6),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Wrap(
            spacing: StrataSpacing.s2,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              Semantics(
                header: true,
                child: Text(title, style: text.bodyStrong),
              ),
              if (note != null)
                Text(note, style: text.caption.copyWith(color: colors.text2)),
              ?end,
            ],
          ),
          const SizedBox(height: StrataSpacing.s2),
          ...children,
        ],
      ),
    );
  }
}

/// Loading, error and not-found states of a page.
class PageLoading extends StatelessWidget {
  /// Creates the loading state.
  const new({super.key});

  @override
  Widget build(BuildContext context) => Center(
    child: Semantics(
      label: context.docsL10n.loading,
      child: const SizedBox.square(
        dimension: 32,
        child: CircularProgressIndicator(strokeWidth: 3),
      ),
    ),
  );
}

/// The error state of a page.
class PageError extends StatelessWidget {
  /// Creates the error state for [error].
  const new({required this.error, super.key});

  /// The core's error.
  final Object error;

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final failure = error;
    return StrataEmptyState(
      icon: Icons.error_outline,
      title: l10n.errorTitle,
      message: l10n.errorMessage(
        code: failure is CoreFailure ? failure.code : 'unknown',
      ),
    );
  }
}

/// The not-found state of a page.
class PageNotFound extends StatelessWidget {
  /// Creates the state.
  const new({super.key});

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    return StrataEmptyState(
      icon: Icons.search_off,
      title: l10n.notFoundTitle,
      message: l10n.notFoundMessage,
    );
  }
}

/// The adaptive frame of an entity-style page:
///
/// * compact: its own top bar (back, section, actions menu) and one scrolling
///   column with the context sections appended;
/// * medium: a header row (breadcrumb, actions, context toggle) over the
///   page; the context panel is an overlay drawer;
/// * expanded: header + page, and the context panel (340) beside it.
class DetailLayout extends StatefulWidget {
  /// Creates the frame.
  const new({
    required this.sectionLabel,
    required this.title,
    required this.main,
    super.key,
    this.backLabel,
    this.subtitle,
    this.actions = const [],
    this.menu = const [],
    this.contextPanel = const [],
    this.compactFooter,
  });

  /// The list this page belongs to ("Documents", "People").
  final String sectionLabel;

  /// Page title (breadcrumb end).
  final String title;

  /// Mono subtitle in the breadcrumb row (e.g. the vault path).
  final String? subtitle;

  /// Accessibility label of the compact back button.
  final String? backLabel;

  /// Header buttons (medium and expanded).
  final List<Widget> actions;

  /// Items of the "More actions" menu.
  final List<PopupMenuEntry<VoidCallback>> menu;

  /// Page content.
  final List<Widget> main;

  /// Context panel sections.
  final List<Widget> contextPanel;

  /// A bar pinned under the compact page (primary action).
  final Widget? compactFooter;

  @override
  State<DetailLayout> createState() => _DetailLayoutState();
}

class _DetailLayoutState extends State<DetailLayout> {
  bool _contextOpen = false;

  Widget _menuButton(BuildContext context) => PopupMenuButton<VoidCallback>(
    tooltip: context.docsL10n.moreActions,
    icon: const Icon(Icons.more_vert),
    onSelected: (action) => action(),
    itemBuilder: (context) => widget.menu,
  );

  @override
  Widget build(BuildContext context) {
    final l10n = context.docsL10n;
    final colors = context.strataColors;
    final text = context.strataText;
    final links = EntityLinks.of(context);
    final sizeClass = SizeClass.of(context);
    const pagePadding = EdgeInsets.fromLTRB(
      StrataSpacing.s5,
      StrataSpacing.s2,
      StrataSpacing.s5,
      StrataSpacing.s8,
    );
    if (sizeClass == SizeClass.compact) {
      final back = links.onBack;
      final footer = widget.compactFooter;
      return ColoredBox(
        color: colors.background,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: StrataSpacing.s1),
              child: Row(
                children: [
                  if (back != null)
                    IconButton(
                      tooltip: widget.backLabel,
                      onPressed: back,
                      icon: const BackButtonIcon(),
                    ),
                  Expanded(
                    child: Padding(
                      padding: const EdgeInsetsDirectional.only(
                        start: StrataSpacing.s2,
                      ),
                      child: Text(
                        widget.sectionLabel,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: text.body.copyWith(color: colors.text2),
                      ),
                    ),
                  ),
                  if (widget.menu.isNotEmpty) _menuButton(context),
                ],
              ),
            ),
            Expanded(
              child: ListView(
                padding: const EdgeInsets.fromLTRB(
                  StrataSpacing.s4,
                  0,
                  StrataSpacing.s4,
                  StrataSpacing.s8,
                ),
                children: [...widget.main, ...widget.contextPanel],
              ),
            ),
            if (footer != null)
              DecoratedBox(
                decoration: BoxDecoration(
                  color: colors.surface,
                  border: Border(top: BorderSide(color: colors.border)),
                ),
                child: SafeArea(
                  top: false,
                  child: Padding(
                    padding: const EdgeInsets.all(StrataSpacing.s3),
                    child: footer,
                  ),
                ),
              ),
          ],
        ),
      );
    }
    final expanded = sizeClass == SizeClass.expanded;
    final subtitle = widget.subtitle;
    final header = Padding(
      padding: const EdgeInsets.symmetric(
        horizontal: StrataSpacing.s5,
        vertical: StrataSpacing.s2,
      ),
      child: Row(
        children: [
          Expanded(
            child: Semantics(
              label: l10n.breadcrumb,
              container: true,
              explicitChildNodes: true,
              child: Wrap(
                crossAxisAlignment: WrapCrossAlignment.center,
                spacing: StrataSpacing.s2,
                children: [
                  if (links.onBack != null)
                    TextButton(
                      onPressed: links.onBack,
                      child: Text(widget.sectionLabel),
                    )
                  else
                    Text(
                      widget.sectionLabel,
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                  ExcludeSemantics(
                    child: Text(
                      '›',
                      style: text.bodySmall.copyWith(color: colors.text2),
                    ),
                  ),
                  Text(
                    widget.title,
                    style: text.bodySmall.withWeight(FontWeight.w600),
                  ),
                  if (subtitle != null)
                    Text(
                      subtitle,
                      textDirection: TextDirection.ltr,
                      style: text.monoSmall.copyWith(color: colors.text2),
                    ),
                ],
              ),
            ),
          ),
          ...widget.actions,
          if (!expanded && widget.contextPanel.isNotEmpty)
            IconButton(
              tooltip: l10n.showContext,
              onPressed: () => setState(() => _contextOpen = true),
              icon: const Icon(Icons.view_sidebar_outlined),
            ),
          if (widget.menu.isNotEmpty) _menuButton(context),
        ],
      ),
    );
    final page = ColoredBox(
      color: colors.surface,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          header,
          Divider(height: 1, color: colors.border),
          Expanded(
            child: ListView(padding: pagePadding, children: widget.main),
          ),
        ],
      ),
    );
    final panel = Semantics(
      container: true,
      explicitChildNodes: true,
      label: l10n.contextLabel,
      child: ColoredBox(
        color: colors.background,
        child: ListView(
          padding: const EdgeInsets.all(StrataSpacing.s4),
          children: widget.contextPanel,
        ),
      ),
    );
    if (expanded) {
      if (widget.contextPanel.isEmpty) return page;
      return Row(
        children: [
          Expanded(child: page),
          VerticalDivider(width: 1, color: colors.border),
          SizedBox(width: StrataLayout.contextPanelWidth, child: panel),
        ],
      );
    }
    return Stack(
      children: [
        Positioned.fill(child: page),
        if (_contextOpen) ...[
          Positioned.fill(
            child: ModalBarrier(
              color: colors.scrim,
              semanticsLabel: l10n.close,
              onDismiss: () => setState(() => _contextOpen = false),
            ),
          ),
          PositionedDirectional(
            top: 0,
            bottom: 0,
            end: 0,
            width: StrataLayout.contextPanelWidth,
            child: DecoratedBox(
              decoration: const BoxDecoration(
                boxShadow: StrataElevation.popover,
              ),
              child: Material(type: MaterialType.transparency, child: panel),
            ),
          ),
        ],
      ],
    );
  }
}
