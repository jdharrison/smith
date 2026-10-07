import 'package:flutter/material.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/halo_presentation.dart';
import 'package:smith_client/home/halo_theme.dart';

class TasksTab extends StatefulWidget {
  const TasksTab({
    super.key,
    required this.chat,
    this.onOpenChat,
    this.wide = false,
  });

  final ChatController chat;
  final VoidCallback? onOpenChat;
  final bool wide;

  @override
  State<TasksTab> createState() => _TasksTabState();
}

class _TasksTabState extends State<TasksTab> {
  WorkFilter _filter = WorkFilter.all;

  @override
  Widget build(BuildContext context) {
    final tasks = widget.chat.tasks
        .where((task) => taskMatchesFilter(task, _filter))
        .toList();
    final selected = widget.chat.selectedTask;

    if (!widget.wide && selected != null) {
      return TaskDetail(
        task: selected,
        onBack: () => widget.chat.selectTask(null),
        updatedAt: widget.chat.tasksUpdatedAt,
        refreshError: widget.chat.tasksError,
        detailError: widget.chat.taskDetailError,
      );
    }

    final list = _TaskList(
      tasks: tasks,
      selectedId: selected?.id,
      filter: _filter,
      updatedAt: widget.chat.tasksUpdatedAt,
      refreshError: widget.chat.tasksError,
      onFilter: (value) => setState(() => _filter = value),
      onRefresh: widget.chat.refreshTasks,
      onSelect: (task) {
        widget.chat.selectTask(task);
        widget.chat.openTaskDetail(task.id);
      },
      onOpenChat: widget.onOpenChat,
    );

    if (!widget.wide) return list;
    return Row(
      children: [
        SizedBox(width: 420, child: list),
        const VerticalDivider(width: 1),
        Expanded(
          child: selected == null
              ? const _NoTaskSelected()
              : TaskDetail(
                  task: selected,
                  updatedAt: widget.chat.tasksUpdatedAt,
                  refreshError: widget.chat.tasksError,
                  detailError: widget.chat.taskDetailError,
                ),
        ),
      ],
    );
  }
}

class _TaskList extends StatelessWidget {
  const _TaskList({
    required this.tasks,
    required this.selectedId,
    required this.filter,
    required this.updatedAt,
    required this.refreshError,
    required this.onFilter,
    required this.onRefresh,
    required this.onSelect,
    this.onOpenChat,
  });

  final List<BoardTask> tasks;
  final String? selectedId;
  final WorkFilter filter;
  final DateTime? updatedAt;
  final String? refreshError;
  final ValueChanged<WorkFilter> onFilter;
  final VoidCallback onRefresh;
  final ValueChanged<BoardTask> onSelect;
  final VoidCallback? onOpenChat;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(24, 20, 24, 12),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Text('Tasks', style: Theme.of(context).textTheme.titleLarge),
                  const Spacer(),
                  IconButton(
                    tooltip: 'Refresh tasks',
                    onPressed: onRefresh,
                    icon: const Icon(Icons.refresh),
                  ),
                ],
              ),
              Row(
                children: [
                  const Icon(
                    Icons.access_time,
                    color: HaloColors.secondary,
                    size: 15,
                  ),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      refreshError != null
                          ? 'Task data may be stale · refresh failed'
                          : updatedAt == null
                          ? 'Task freshness unknown'
                          : 'Updated ${_formatTime(updatedAt!)}',
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: refreshError != null
                            ? HaloColors.warning
                            : HaloColors.secondary,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 16),
              SingleChildScrollView(
                scrollDirection: Axis.horizontal,
                child: Row(
                  children: WorkFilter.values
                      .map(
                        (value) => Padding(
                          padding: const EdgeInsets.only(right: 8),
                          child: FilterChip(
                            selected: value == filter,
                            onSelected: (_) => onFilter(value),
                            label: Text(workFilterLabel(value)),
                            showCheckmark: false,
                            side: const BorderSide(color: HaloColors.border),
                            selectedColor: HaloColors.accent.withValues(
                              alpha: 0.14,
                            ),
                            backgroundColor: HaloColors.surface,
                            labelStyle: TextStyle(
                              color: value == filter
                                  ? HaloColors.accent
                                  : HaloColors.primary,
                            ),
                          ),
                        ),
                      )
                      .toList(),
                ),
              ),
            ],
          ),
        ),
        Expanded(
          child: tasks.isEmpty
              ? _EmptyTasks(filter: filter, onOpenChat: onOpenChat)
              : ListView.separated(
                  padding: const EdgeInsets.fromLTRB(24, 4, 24, 24),
                  itemCount: tasks.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 8),
                  itemBuilder: (context, index) {
                    final task = tasks[index];
                    return TaskListRow(
                      task: task,
                      selected: selectedId == task.id,
                      onTap: () => onSelect(task),
                    );
                  },
                ),
        ),
      ],
    );
  }

  static String _formatTime(DateTime value) {
    final hour = value.hour.toString().padLeft(2, '0');
    final minute = value.minute.toString().padLeft(2, '0');
    final second = value.second.toString().padLeft(2, '0');
    return '$hour:$minute:$second';
  }
}

class TaskListRow extends StatelessWidget {
  const TaskListRow({
    super.key,
    required this.task,
    required this.selected,
    required this.onTap,
  });

  final BoardTask task;
  final bool selected;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Semantics(
      selected: selected,
      button: true,
      label:
          '${task.title}, ${taskStatusLabel(task.status)}, ${taskStageLabel(task.stage)}',
      child: InkWell(
        onTap: onTap,
        borderRadius: BorderRadius.circular(12),
        child: AnimatedContainer(
          duration: MediaQuery.disableAnimationsOf(context)
              ? Duration.zero
              : const Duration(milliseconds: 140),
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            color: selected
                ? HaloColors.accent.withValues(alpha: 0.08)
                : HaloColors.surface,
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
              color: selected ? HaloColors.accent : HaloColors.border,
            ),
          ),
          child: Row(
            children: [
              _StatusIcon(status: task.status),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      task.title.isEmpty ? 'Untitled task' : task.title,
                      maxLines: 2,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                        fontWeight: FontWeight.w600,
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      '${taskStatusLabel(task.status)} · ${taskStageLabel(task.stage)}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                        color: HaloColors.secondary,
                      ),
                    ),
                  ],
                ),
              ),
              const Icon(Icons.chevron_right, color: HaloColors.secondary),
            ],
          ),
        ),
      ),
    );
  }
}

class TaskDetail extends StatelessWidget {
  const TaskDetail({
    super.key,
    required this.task,
    this.onBack,
    this.updatedAt,
    this.refreshError,
    this.detailError,
  });

  final BoardTask task;
  final VoidCallback? onBack;
  final DateTime? updatedAt;
  final String? refreshError;
  final String? detailError;

  @override
  Widget build(BuildContext context) {
    final hasResult = task.resultText.trim().isNotEmpty;
    return ListView(
      padding: const EdgeInsets.fromLTRB(24, 20, 24, 40),
      children: [
        if (onBack != null)
          Align(
            alignment: Alignment.centerLeft,
            child: TextButton.icon(
              onPressed: onBack,
              icon: const Icon(Icons.arrow_back),
              label: const Text('Back to Work'),
            ),
          ),
        if (refreshError != null || detailError != null)
          _UnavailableNotice(
            text: detailError != null
                ? 'Task details could not be refreshed: $detailError'
                : 'Task data may be stale: $refreshError',
          )
        else if (updatedAt != null)
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Text(
              'Task snapshot refreshed at ${updatedAt!.toLocal()}',
              style: const TextStyle(color: HaloColors.secondary, fontSize: 13),
            ),
          ),
        Wrap(
          spacing: 12,
          runSpacing: 8,
          crossAxisAlignment: WrapCrossAlignment.center,
          children: [
            Text(
              task.title.isEmpty ? 'Untitled task' : task.title,
              style: Theme.of(context).textTheme.headlineSmall,
            ),
            TaskStatusBadge(status: task.status),
          ],
        ),
        const SizedBox(height: 8),
        SelectableText(
          task.id,
          style: Theme.of(
            context,
          ).textTheme.bodySmall?.copyWith(color: HaloColors.secondary),
        ),
        const SizedBox(height: 28),
        Text(
          'TRACE',
          style: Theme.of(context).textTheme.labelSmall?.copyWith(
            color: HaloColors.secondary,
            letterSpacing: 1.3,
          ),
        ),
        const SizedBox(height: 12),
        _Trace(task: task),
        const SizedBox(height: 32),
        _DetailSection(
          title: 'Request',
          body: task.userText.isEmpty
              ? 'Request details unavailable.'
              : task.userText,
        ),
        _DetailSection(
          title: 'Run',
          body: task.jobId == null
              ? 'No pipeline run identifier is available for this task.'
              : '${task.jobId}\n${taskStageLabel(task.stage)} · ${taskStatusLabel(task.status)}',
        ),
        _DetailSection(
          title: 'Result',
          body: hasResult
              ? task.resultText
              : task.error?.trim().isNotEmpty == true
              ? 'Failed: ${task.error}'
              : task.expectedResult.isNotEmpty
              ? 'Expected: ${task.expectedResult}\n\nA verified result is not available yet.'
              : 'A verified result is not available yet.',
          tone: task.status == 'failed' ? HaloColors.danger : null,
        ),
        if (task.status == 'awaiting_signoff')
          const _UnavailableNotice(
            text:
                'Approval is unavailable in this client. Review and authorize through the currently approved operator path.',
          ),
        const SizedBox(height: 8),
        Wrap(
          spacing: 12,
          runSpacing: 12,
          children: [
            Tooltip(
              message: hasResult
                  ? 'The available result is shown above; no artifact reference was supplied'
                  : 'No reviewable artifact reference is available',
              child: FilledButton.icon(
                onPressed: null,
                icon: const Icon(Icons.open_in_new),
                label: const Text('Review result'),
              ),
            ),
            OutlinedButton.icon(
              onPressed: () => showDialog<void>(
                context: context,
                builder: (context) => _ActivityDialog(task: task),
              ),
              icon: const Icon(Icons.history),
              label: const Text('View activity'),
            ),
          ],
        ),
      ],
    );
  }
}

class _Trace extends StatelessWidget {
  const _Trace({required this.task});

  final BoardTask task;

  @override
  Widget build(BuildContext context) {
    return Wrap(
      spacing: 8,
      runSpacing: 8,
      crossAxisAlignment: WrapCrossAlignment.center,
      children: [
        _TraceNode(label: 'Task', value: task.id),
        const Icon(Icons.arrow_forward, size: 18, color: HaloColors.secondary),
        _TraceNode(label: 'Run', value: task.jobId ?? 'Unavailable'),
        const Icon(Icons.arrow_forward, size: 18, color: HaloColors.secondary),
        _TraceNode(
          label: 'Result',
          value: task.resultText.isEmpty ? 'Unavailable' : 'Reported',
        ),
      ],
    );
  }
}

class _TraceNode extends StatelessWidget {
  const _TraceNode({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Container(
      constraints: const BoxConstraints(minWidth: 112),
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: HaloColors.surface,
        border: Border.all(color: HaloColors.border),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(label, style: const TextStyle(color: HaloColors.secondary)),
          const SizedBox(height: 3),
          Text(value, maxLines: 1, overflow: TextOverflow.ellipsis),
        ],
      ),
    );
  }
}

class _DetailSection extends StatelessWidget {
  const _DetailSection({required this.title, required this.body, this.tone});

  final String title;
  final String body;
  final Color? tone;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 28),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: Theme.of(
              context,
            ).textTheme.titleMedium?.copyWith(fontWeight: FontWeight.w600),
          ),
          const SizedBox(height: 8),
          SelectableText(
            body,
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
              height: 1.5,
              color: tone ?? HaloColors.secondary,
            ),
          ),
        ],
      ),
    );
  }
}

class TaskStatusBadge extends StatelessWidget {
  const TaskStatusBadge({super.key, required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final color = switch (status) {
      'failed' => HaloColors.danger,
      'awaiting_signoff' => HaloColors.warning,
      'running' => HaloColors.info,
      'completed' => HaloColors.accent,
      _ => HaloColors.secondary,
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 6),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.10),
        border: Border.all(color: color.withValues(alpha: 0.55)),
        borderRadius: BorderRadius.circular(999),
      ),
      child: Text(
        taskStatusLabel(status),
        style: TextStyle(color: color, fontSize: 13),
      ),
    );
  }
}

class _StatusIcon extends StatelessWidget {
  const _StatusIcon({required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final (icon, color) = switch (status) {
      'failed' => (Icons.error_outline, HaloColors.danger),
      'awaiting_signoff' => (Icons.rate_review_outlined, HaloColors.warning),
      'running' => (Icons.play_circle_outline, HaloColors.info),
      'queued' => (Icons.schedule, HaloColors.secondary),
      'completed' => (Icons.check_circle_outline, HaloColors.accent),
      _ => (Icons.help_outline, HaloColors.secondary),
    };
    return Icon(icon, color: color, size: 24);
  }
}

class _UnavailableNotice extends StatelessWidget {
  const _UnavailableNotice({required this.text});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 24),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: HaloColors.warning.withValues(alpha: 0.08),
        border: Border.all(color: HaloColors.warning.withValues(alpha: 0.45)),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Icon(Icons.info_outline, color: HaloColors.warning),
          const SizedBox(width: 12),
          Expanded(child: Text(text)),
        ],
      ),
    );
  }
}

class _ActivityDialog extends StatelessWidget {
  const _ActivityDialog({required this.task});

  final BoardTask task;

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Available activity'),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 480),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text('Created: ${_formatEpoch(task.createdAt)}'),
            const SizedBox(height: 8),
            Text('Updated: ${_formatEpoch(task.updatedAt)}'),
            const SizedBox(height: 16),
            const Text(
              'Timestamped run events and verification evidence are not supplied by the current bridge contract.',
              style: TextStyle(color: HaloColors.secondary),
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Close'),
        ),
      ],
    );
  }

  static String _formatEpoch(int seconds) {
    if (seconds <= 0) return 'Unavailable';
    return DateTime.fromMillisecondsSinceEpoch(
      seconds * 1000,
    ).toLocal().toString();
  }
}

class _EmptyTasks extends StatelessWidget {
  const _EmptyTasks({required this.filter, this.onOpenChat});

  final WorkFilter filter;
  final VoidCallback? onOpenChat;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(
              Icons.work_outline,
              color: HaloColors.secondary,
              size: 36,
            ),
            const SizedBox(height: 12),
            Text(
              filter == WorkFilter.all
                  ? 'No tasks are available.'
                  : 'No ${workFilterLabel(filter).toLowerCase()} tasks.',
            ),
            const SizedBox(height: 6),
            const Text(
              'Create bounded work through Chat.',
              style: TextStyle(color: HaloColors.secondary),
            ),
            if (onOpenChat != null) ...[
              const SizedBox(height: 12),
              TextButton(onPressed: onOpenChat, child: const Text('Open Chat')),
            ],
          ],
        ),
      ),
    );
  }
}

class _NoTaskSelected extends StatelessWidget {
  const _NoTaskSelected();

  @override
  Widget build(BuildContext context) {
    return const Center(
      child: Text(
        'Select a task to inspect its available trace and result.',
        style: TextStyle(color: HaloColors.secondary),
      ),
    );
  }
}
