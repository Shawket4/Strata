import 'dart:async';

import 'package:alchemist/alchemist.dart';
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:strata_ui/strata_ui.dart';
import 'package:strata_ui/testing.dart';

import '../helpers/gallery.dart';
import '../helpers/harness.dart';

/// Design-system widgets (brand marks, relation chips, node glyphs, sync and
/// status pills, citations, keyboard hints, section headers, empty state) in
/// light/dark × LTR/RTL × text scale 1.0/2.0. Component rendering does not
/// depend on the window size class, so one frame width is used.
void main() {
  setUpAll(loadStrataFonts);

  group('design system goldens', () {
    for (final v in variants(sizes: const {'gallery': Size(440, 1600)})) {
      unawaited(
        goldenTest(
          'widget gallery $v',
          fileName: 'design_system_${v.id}',
          builder: () => goldenFrame(
            v,
            const Material(
              type: MaterialType.transparency,
              child: Padding(
                padding: EdgeInsets.all(StrataSpacing.s4),
                child: WidgetGallery(),
              ),
            ),
            intrinsicHeight: true,
          ),
        ),
      );
    }
  });
}
