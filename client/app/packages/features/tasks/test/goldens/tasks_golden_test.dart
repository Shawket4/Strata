@Tags(['golden'])
library;

import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_state/strata_state.dart';
import 'package:strata_state/testing.dart';
import 'package:strata_tasks/strata_tasks.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/harness.dart';

FakeCoreApi _tasksFake() => FakeCoreApi()
  ..tasks.add(StrataFixtures.tasksView)
  ..task['t-watanya-eta'].add(StrataFixtures.taskScreen)
  ..duplicatePrompts.add(StrataFixtures.duplicatePromptsView);

/// Tasks, task detail, the new-task sheet and the "Already exists" prompt at
/// compact / medium / expanded × light/dark × LTR/RTL (1.0; compact also at
/// 2.0), rendered with the bundled fonts.
void main() {
  setUpAll(loadStrataFonts);

  for (final v in goldenMatrix()) {
    unawaited(
      goldenTest(
        'tasks screen $v',
        fileName: 'tasks_screen_${v.id}',
        builder: () => goldenScreen(
          v,
          TasksScreen(
            initialTaskId: v.sizeClass == SizeClass.compact
                ? null
                : 't-watanya-eta',
          ),
          _tasksFake(),
        ),
      ),
    );
    unawaited(
      goldenTest(
        'task detail $v',
        fileName: 'task_detail_${v.id}',
        builder: () => goldenScreen(
          v,
          const TaskDetailScreen(taskId: 't-watanya-eta'),
          _tasksFake(),
          scaffold: false,
        ),
      ),
    );
    unawaited(
      goldenTest(
        'task editor $v',
        fileName: 'task_editor_${v.id}',
        builder: () => goldenScreen(v, const TaskEditorSheet(), _tasksFake()),
      ),
    );
    unawaited(
      goldenTest(
        'duplicate prompt $v',
        fileName: 'duplicate_prompt_${v.id}',
        builder: () => goldenScreen(
          v,
          const SingleChildScrollView(child: DuplicatePromptSheet()),
          _tasksFake(),
        ),
      ),
    );
  }
  for (final v in goldenMatrix(
    sizes: const {'compact': StrataTestSizes.compact},
  )) {
    unawaited(
      goldenTest(
        'tasks empty $v',
        fileName: 'tasks_empty_${v.id}',
        builder: () => goldenScreen(
          v,
          const TasksScreen(),
          FakeCoreApi()
            ..tasks.add(
              const TasksView(
                sections: TaskSections(
                  overdue: [],
                  today: [],
                  upcoming: [],
                  recurring: [],
                  noDate: [],
                ),
                done: [],
              ),
            ),
        ),
      ),
    );
  }
}
