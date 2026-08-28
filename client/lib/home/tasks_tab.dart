import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:smith_client/bridge/smith_bridge.dart';
import 'package:smith_client/chat/chat_controller.dart';

/// Tasks board tab — list + optional detail.
class TasksTab extends StatelessWidget {
  const TasksTab({super.key, required this.chat});

  final ChatController chat;

  static const _ink = Color(0xFF12353F);
  static const _muted = Color(0xFF6A7C86);
  static const _border = Color(0xFFD0D8DE);

  @override
  Widget build(BuildContext context) {
    final selected = chat.selectedTask;
    if (selected != null) {
      return _TaskDetail(
        task: selected,
        onBack: () => chat.selectTask(null),
      );
    }

    final tasks = chat.tasks;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(16, 4, 16, 8),
          child: Row(
            children: [
              Text(
                'Board',
                style: GoogleFonts.dmSans(
                  fontSize: 13,
                  fontWeight: FontWeight.w600,
                  color: _muted,
                ),
              ),
              const Spacer(),
              TextButton(
                onPressed: () => chat.refreshTasks(),
                child: const Text('Refresh'),
              ),
            ],
          ),
        ),
        Expanded(
          child: tasks.isEmpty
              ? Center(
                  child: Text(
                    'No tasks yet — send a chat message.',
                    style: GoogleFonts.dmSans(fontSize: 14, color: _muted),
                  ),
                )
              : ListView.separated(
                  padding: const EdgeInsets.fromLTRB(16, 0, 16, 24),
                  itemCount: tasks.length,
                  separatorBuilder: (_, _) => const SizedBox(height: 8),
                  itemBuilder: (context, i) {
                    final t = tasks[i];
                    return InkWell(
                      onTap: () => chat.openTaskDetail(t.id),
                      borderRadius: BorderRadius.circular(8),
                      child: Container(
                        padding: const EdgeInsets.all(12),
                        decoration: BoxDecoration(
                          border: Border.all(color: _border),
                          borderRadius: BorderRadius.circular(8),
                          color: Colors.white,
                        ),
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Row(
                              children: [
                                _StatusChip(status: t.status),
                                const SizedBox(width: 8),
                                Text(
                                  t.kind,
                                  style: GoogleFonts.dmSans(
                                    fontSize: 11,
                                    color: _muted,
                                  ),
                                ),
                                const Spacer(),
                                if (t.isRunning)
                                  const SizedBox(
                                    width: 12,
                                    height: 12,
                                    child: CircularProgressIndicator(
                                      strokeWidth: 2,
                                    ),
                                  ),
                              ],
                            ),
                            const SizedBox(height: 6),
                            Text(
                              t.title,
                              maxLines: 2,
                              overflow: TextOverflow.ellipsis,
                              style: GoogleFonts.dmSans(
                                fontSize: 14,
                                fontWeight: FontWeight.w600,
                                color: _ink,
                              ),
                            ),
                            if (t.snippet.isNotEmpty) ...[
                              const SizedBox(height: 4),
                              Text(
                                t.snippet,
                                maxLines: 2,
                                overflow: TextOverflow.ellipsis,
                                style: GoogleFonts.dmSans(
                                  fontSize: 12,
                                  height: 1.35,
                                  color: _muted,
                                ),
                              ),
                            ],
                          ],
                        ),
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _StatusChip extends StatelessWidget {
  const _StatusChip({required this.status});

  final String status;

  @override
  Widget build(BuildContext context) {
    final color = switch (status) {
      'completed' => const Color(0xFF1FA88A),
      'failed' => const Color(0xFFB04A4A),
      'awaiting_signoff' => const Color(0xFFC48A2A),
      'running' || 'queued' => const Color(0xFF2F6FED),
      _ => const Color(0xFF6A7C86),
    };
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.12),
        borderRadius: BorderRadius.circular(4),
      ),
      child: Text(
        status,
        style: GoogleFonts.dmSans(
          fontSize: 10,
          fontWeight: FontWeight.w600,
          color: color,
        ),
      ),
    );
  }
}

class _TaskDetail extends StatelessWidget {
  const _TaskDetail({required this.task, required this.onBack});

  final BoardTask task;
  final VoidCallback onBack;

  static const _ink = Color(0xFF12353F);
  static const _muted = Color(0xFF6A7C86);

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.fromLTRB(16, 4, 16, 32),
      children: [
        Align(
          alignment: Alignment.centerLeft,
          child: TextButton.icon(
            onPressed: onBack,
            icon: const Icon(Icons.arrow_back, size: 16),
            label: const Text('Tasks'),
          ),
        ),
        const SizedBox(height: 4),
        Row(
          children: [
            _StatusChip(status: task.status),
            const SizedBox(width: 8),
            Text(
              '${task.kind} · ${task.stage}',
              style: GoogleFonts.dmSans(fontSize: 12, color: _muted),
            ),
          ],
        ),
        const SizedBox(height: 10),
        Text(
          task.title,
          style: GoogleFonts.dmSans(
            fontSize: 18,
            fontWeight: FontWeight.w600,
            color: _ink,
          ),
        ),
        const SizedBox(height: 16),
        _field('id', task.id),
        if (task.jobId != null) _field('job', task.jobId!),
        _field('user', task.userText),
        if (task.ackText.isNotEmpty) _field('ack', task.ackText),
        if (task.expectedResult.isNotEmpty)
          _field('expected', task.expectedResult),
        if (task.resultText.isNotEmpty) _field('result', task.resultText),
        if (task.error != null) _field('error', task.error!),
      ],
    );
  }

  Widget _field(String label, String value) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            label,
            style: GoogleFonts.dmSans(
              fontSize: 11,
              fontWeight: FontWeight.w600,
              letterSpacing: 0.4,
              color: _muted,
            ),
          ),
          const SizedBox(height: 4),
          SelectableText(
            value,
            style: GoogleFonts.dmSans(
              fontSize: 14,
              height: 1.4,
              color: _ink,
            ),
          ),
        ],
      ),
    );
  }
}
