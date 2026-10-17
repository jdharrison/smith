import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../workspace_controller.dart';
import 'smith_theme.dart';

enum WorkspaceLayout {
  single(1, 1),
  sideBySide(2, 1),
  stacked(1, 2),
  grid(2, 2);

  const WorkspaceLayout(this.columns, this.rows);
  final int columns;
  final int rows;
  int get count => columns * rows;
  String get label => '$columns×$rows';
}

/// One native menu trigger controls the window-wide grid.
class WorkspaceLayoutMenu extends StatelessWidget {
  const WorkspaceLayoutMenu({
    super.key,
    required this.layout,
    required this.onSelected,
  });

  final WorkspaceLayout layout;
  final ValueChanged<WorkspaceLayout> onSelected;

  @override
  Widget build(BuildContext context) {
    return PopupMenuButton<WorkspaceLayout>(
      key: const ValueKey('workspace-layout'),
      tooltip: 'Workspace layout (columns × rows)',
      initialValue: layout,
      onSelected: onSelected,
      icon: const Icon(Icons.grid_view_outlined),
      style: ButtonStyle(
        // Focus keeps the theme's visible outline, not a hover-like fill.
        overlayColor: WidgetStateProperty.resolveWith((states) {
          if (states.contains(WidgetState.pressed)) {
            return SmithColors.accent.withValues(alpha: .18);
          }
          if (states.contains(WidgetState.hovered)) {
            return SmithColors.accent.withValues(alpha: .10);
          }
          return Colors.transparent;
        }),
      ),
      itemBuilder: (_) => [
        for (final value in WorkspaceLayout.values)
          CheckedPopupMenuItem(
            value: value,
            checked: value == layout,
            child: Text(
              value.label,
              semanticsLabel: '${value.columns} columns × ${value.rows} rows',
            ),
          ),
      ],
    );
  }
}

/// Presentation only: the application retains ownership of the shared service.
class SplitWorkspace extends StatefulWidget {
  const SplitWorkspace({
    super.key,
    required this.controller,
    required this.builder,
    required this.headerBuilder,
    this.initialLayout = WorkspaceLayout.single,
    this.initialActive = 0,
    this.onPresentationChanged,
  });
  final Widget Function(BuildContext, Widget) headerBuilder;
  final WorkspaceLayout initialLayout;
  final int initialActive;
  final void Function(WorkspaceLayout, int)? onPresentationChanged;
  final WorkspaceController controller;
  final Widget Function(BuildContext, WorkspacePane, bool, VoidCallback)
  builder;

  @override
  State<SplitWorkspace> createState() => _SplitWorkspaceState();
}

class _SplitWorkspaceState extends State<SplitWorkspace> {
  late WorkspaceLayout layout;
  late int active;

  @override
  void initState() {
    super.initState();
    layout = widget.initialLayout;
    active = widget.initialActive;
  }

  final horizontal = ScrollController();
  final vertical = ScrollController();
  // PopupMenuButton's native icon path owns its button focus node. Retain
  // its scope so shrinking the grid can restore that same focused child.
  final layoutFocus = FocusScopeNode(
    debugLabel: 'Workspace layout',
    traversalEdgeBehavior: TraversalEdgeBehavior.parentScope,
  );

  void choose(WorkspaceLayout value) {
    while (widget.controller.panes.length < value.count) {
      widget.controller.createPane();
    }
    if (active >= value.count) {
      FocusManager.instance.primaryFocus?.unfocus();
      WidgetsBinding.instance.addPostFrameCallback((_) {
        if (!mounted) return;
        FocusTraversalGroup.of(layoutFocus.context!)
            .findFirstFocus(layoutFocus, ignoreCurrentFocus: true)
            ?.requestFocus();
      });
    }
    setState(() {
      layout = value;
      if (active >= value.count) active = 0;
    });
    widget.controller.setVisiblePanes(
      widget.controller.panes.take(value.count).toSet(),
    );
    widget.onPresentationChanged?.call(layout, active);
    for (final pane in widget.controller.panes.take(value.count)) {
      final selected = pane.selected;
      if (selected.id != null &&
          (!selected.detailLoaded || selected.detailStale)) {
        widget.controller.selectFor(pane, selected.id!);
      }
    }
  }

  void activatePane(int index) {
    if (active == index) return;
    setState(() => active = index);
    widget.onPresentationChanged?.call(layout, active);
  }

  @override
  void dispose() {
    horizontal.dispose();
    vertical.dispose();
    layoutFocus.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Column(
    children: [
      widget.headerBuilder(
        context,
        FocusScope(
          node: layoutFocus,
          child: WorkspaceLayoutMenu(layout: layout, onSelected: choose),
        ),
      ),
      Expanded(child: _grid()),
    ],
  );

  Widget _grid() => LayoutBuilder(
    builder: (context, bounds) {
      final scale = MediaQuery.textScalerOf(context).scale(14) / 14;
      final tileWidth = math.max(
        layout == WorkspaceLayout.single ? 360.0 : 400.0 * math.max(1, scale),
        bounds.maxWidth / layout.columns,
      );
      final tileHeight = math.max(
        layout == WorkspaceLayout.single ? 320.0 : 400.0 * math.max(1, scale),
        bounds.maxHeight / layout.rows,
      );
      final scrollBehavior = ScrollConfiguration.of(context);
      // Suppress desktop duplicates only for the canvas, not its tile contents.
      return ScrollConfiguration(
        behavior: scrollBehavior.copyWith(scrollbars: false),
        child: Scrollbar(
          controller: vertical,
          thumbVisibility: true,
          notificationPredicate: (notification) =>
              notification.metrics.axis == Axis.vertical &&
              notification.depth == 1,
          child: Scrollbar(
            controller: horizontal,
            thumbVisibility: true,
            notificationPredicate: (notification) =>
                notification.metrics.axis == Axis.horizontal &&
                notification.depth == 0,
            child: SingleChildScrollView(
              controller: horizontal,
              scrollDirection: Axis.horizontal,
              child: SizedBox(
                width: tileWidth * layout.columns,
                child: SingleChildScrollView(
                  controller: vertical,
                  child: SizedBox(
                    height: tileHeight * layout.rows,
                    child: Stack(
                      children: [
                        for (var i = 0; i < widget.controller.panes.length; i++)
                          Positioned(
                            key: ObjectKey(widget.controller.panes[i]),
                            left: (i % layout.columns) * tileWidth,
                            top: (i ~/ layout.columns) * tileHeight,
                            width: tileWidth,
                            height: tileHeight,
                            child: Offstage(
                              offstage: i >= layout.count,
                              child: ExcludeFocus(
                                excluding: i >= layout.count,
                                child: TickerMode(
                                  enabled: i < layout.count,
                                  child: FocusTraversalGroup(
                                    child: Focus(
                                      onFocusChange: (focused) {
                                        if (focused && i < layout.count) {
                                          activatePane(i);
                                        }
                                      },
                                      child: Semantics(
                                        container: true,
                                        label: 'Workspace ${i + 1}',
                                        child: Container(
                                          key: ValueKey('workspace-tile-$i'),
                                          decoration: BoxDecoration(
                                            border: Border.all(
                                              color: SmithColors.strokeSubtle,
                                            ),
                                          ),
                                          child: ScrollConfiguration(
                                            behavior: scrollBehavior,
                                            child: widget.builder(
                                              context,
                                              widget.controller.panes[i],
                                              active == i && i < layout.count,
                                              () => activatePane(i),
                                            ),
                                          ),
                                        ),
                                      ),
                                    ),
                                  ),
                                ),
                              ),
                            ),
                          ),
                      ],
                    ),
                  ),
                ),
              ),
            ),
          ),
        ),
      );
    },
  );
}
