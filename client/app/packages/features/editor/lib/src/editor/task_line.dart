import 'package:flutter/material.dart';
import 'package:strata_editor/src/editor/note_editor_controller.dart';
import 'package:strata_editor/src/generated/editor_localizations.dart';
import 'package:strata_state/strata_state.dart' show TaskItem, TaskState;
import 'package:strata_ui/strata_ui.dart';
import 'package:super_editor/super_editor.dart';

/// Renders the core's task lines (`HintKind.taskLine`) with a checkbox in
/// front of the line's source text. The checkbox forwards
/// `completeTask` / `reopenTask` for the task the core knows by the line's
/// block ID; the line text itself stays editable markdown.
final class TaskLineComponentBuilder implements ComponentBuilder {
  /// Creates the builder for [controller]'s task lines.
  const new(this.controller);

  /// The editing session.
  final NoteEditorController controller;

  @override
  SingleColumnLayoutComponentViewModel? createViewModel(
    Document document,
    DocumentNode node,
  ) => null;

  @override
  Widget? createComponent(
    SingleColumnDocumentComponentContext componentContext,
    SingleColumnLayoutComponentViewModel componentViewModel,
  ) {
    if (componentViewModel is! ParagraphComponentViewModel) return null;
    final nodeId = componentViewModel.nodeId;
    if (!controller.lineHints.isTaskLine(nodeId)) return null;
    return TaskLineComponent(
      key: componentContext.componentKey,
      viewModel: componentViewModel,
      task: controller.taskOf(nodeId),
      onToggle: () => controller.toggleTask(nodeId),
    );
  }
}

/// A task line: checkbox + the line's text component.
class TaskLineComponent extends StatefulWidget {
  /// Creates the component.
  const new({
    required this.viewModel,
    required this.task,
    required this.onToggle,
    super.key,
  });

  /// The paragraph view model of the line.
  final ParagraphComponentViewModel viewModel;

  /// The task the core knows for this line (`null`: not known yet).
  final TaskItem? task;

  /// Toggles the task.
  final VoidCallback onToggle;

  @override
  State<TaskLineComponent> createState() => _TaskLineComponentState();
}

class _TaskLineComponentState extends State<TaskLineComponent>
    with ProxyDocumentComponent<TaskLineComponent>, ProxyTextComposable {
  final GlobalKey _textKey = GlobalKey();

  @override
  GlobalKey<State<StatefulWidget>> get childDocumentComponentKey => _textKey;

  @override
  TextComposable get childTextComposable =>
      childDocumentComponentKey.currentState! as TextComposable;

  @override
  Widget build(BuildContext context) {
    final l10n = EditorLocalizations.of(context);
    final vm = widget.viewModel;
    final task = widget.task;
    final label = task == null
        ? l10n.taskNotSynced
        : task.state == TaskState.open
        ? l10n.taskComplete(task: task.description)
        : l10n.taskReopen(task: task.description);
    return Directionality(
      textDirection: vm.textDirection,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Semantics(
            label: label,
            excludeSemantics: true,
            checked: task != null && task.state != TaskState.open,
            enabled: task != null,
            onTap: task == null ? null : widget.onToggle,
            child: Checkbox(
              value: switch (task?.state) {
                null || TaskState.open => false,
                TaskState.done => true,
                TaskState.cancelled => null,
              },
              tristate: task?.state == TaskState.cancelled,
              onChanged: task == null ? null : (_) => widget.onToggle(),
              side: BorderSide(color: context.strataColors.text2, width: 1.5),
            ),
          ),
          const SizedBox(width: StrataSpacing.s1),
          Expanded(
            child: Padding(
              padding: const EdgeInsets.only(top: StrataSpacing.s2 + 2),
              child: TextComponent(
                key: _textKey,
                text: vm.text,
                textDirection: vm.textDirection,
                textAlign: vm.textAlignment,
                textStyleBuilder: vm.textStyleBuilder,
                inlineWidgetBuilders: vm.inlineWidgetBuilders,
                textSelection: vm.selection,
                selectionColor: vm.selectionColor,
                highlightWhenEmpty: vm.highlightWhenEmpty,
                underlines: vm.createUnderlines(),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
