import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';

import 'app_api.dart';
import 'audio.dart';

class ChatController extends ChangeNotifier {
  ChatController(
    this.api,
    this.audio, {
    this.pollInterval = const Duration(milliseconds: 500),
    this.operationBudget = const Duration(seconds: 210),
  });
  final AppApi api;
  final AudioIo audio;
  final Duration pollInterval;
  final Duration operationBudget;
  String draft = '';
  int _draftRevision = 0;
  int _audioRevision = 0;
  bool speech = false;
  bool microphone = false;
  bool recording = false;
  bool busy = false;
  bool playing = false;
  bool _closed = false;
  String phase = 'Ready';
  String? error;
  String? instanceId;
  String? pendingId;
  String? pendingKind;
  bool _allowPlayback = false;
  int _playbackRevision = 0;
  int? _transcriptRevision;
  String? transcriptForInsertion;
  String? _expiredRequestId;
  bool get canAbandonExpiredRequest =>
      pendingId != null && pendingId == _expiredRequestId;
  String? _submittedText;
  int? _submittedRevision;
  Map<String, dynamic> config = {};
  Map<String, dynamic> providers = {};
  List<Map<String, dynamic>> history = [];
  List<String> models = [];
  List<String> voices = [];
  void _notify() {
    if (!_closed) notifyListeners();
  }

  void edit(String value) {
    draft = value;
    _draftRevision++;
    _notify();
  }

  void setSpeech(bool value) {
    speech = value;
    _audioRevision++;
    _allowPlayback = false;
    if (!value && playing) {
      playing = false;
      unawaited(audio.stop().catchError((Object _) {}));
    }
    _notify();
  }

  void insertTranscript() {
    if (_closed || busy || recording || transcriptForInsertion == null) return;
    final transcript = transcriptForInsertion!;
    transcriptForInsertion = null;
    if (error?.startsWith('Transcript arrived after editing;') == true) {
      error = null;
    }
    edit(draft.isEmpty ? transcript : '$draft\n$transcript');
  }

  void abandonExpiredRequest(String requestId) {
    if (_closed ||
        busy ||
        recording ||
        !canAbandonExpiredRequest ||
        pendingId != requestId) {
      return;
    }
    pendingId = null;
    pendingKind = null;
    _expiredRequestId = null;
    _allowPlayback = false;
    _submittedText = null;
    _submittedRevision = null;
    _transcriptRevision = null;
    phase = 'Ready';
    error = 'Expired request forgotten locally. Its outcome may be unknown; review saved history before sending new work. Nothing was resent or cancelled.';
    _notify();
  }

  Future<void> synchronize() async {
    if (busy || _closed) return;
    busy = true;
    error = null;
    _notify();
    try {
      final reply = await api.call('state', {});
      if (_closed) return;
      final old = instanceId;
      instanceId = reply.instanceId;
      _readState(reply.result);
      if (old != null && old != instanceId && pendingId != null) {
        pendingId = null;
        pendingKind = null;
        _allowPlayback = false;
        throw const AppFailure(
          'RESTART',
          'Service restarted. Work is not resent; review saved history.',
        );
      }
      final active = reply.result['active'];
      if (pendingId == null && active is Map<String, dynamic>) {
        pendingId = active['request_id'] as String?;
        pendingKind = active['kind'] as String?;
        _allowPlayback = false;
      }
      if (pendingId != null) await _poll();
    } on Object catch (e) {
      error = e.toString();
    } finally {
      busy = false;
      _notify();
    }
  }

  void _readState(Map<String, dynamic> state) {
    config = Map<String, dynamic>.from(state['config'] as Map);
    history = (state['history'] as List)
        .map((v) => Map<String, dynamic>.from(v as Map))
        .toList();
    providers = Map<String, dynamic>.from(state['providers'] as Map);
  }

  Future<void> configure(String? model, String? voice) async {
    if (busy || pendingId != null || recording || _closed) return;
    busy = true;
    error = null;
    _notify();
    try {
      final reply = await api.call('configure', {
        'model': model,
        'voice': voice,
      });
      _correlate(reply);
      if (!_closed) {
        final state = await api.call('state', {});
        _correlate(state);
        _readState(state.result);
      }
    } on Object catch (e) {
      error = e.toString();
    } finally {
      busy = false;
      _notify();
    }
  }

  void _correlate(AppReply reply) {
    if (instanceId != null && reply.instanceId != instanceId) {
      throw const AppFailure(
        'RESTART',
        'Service instance changed; synchronize, never resend.',
      );
    }
    instanceId = reply.instanceId;
  }

  Future<void> refreshModels() => _operate('models', {});
  Future<void> send() async {
    if (busy ||
        pendingId != null ||
        recording ||
        _closed ||
        draft.trim().isEmpty) {
      return;
    }
    if (utf8.encode(draft).length > 16384) {
      error = 'Draft exceeds 16 KiB.';
      _notify();
      return;
    }
    await _operate('submit', {
      'text': draft,
      'output': speech ? 'speech' : 'chat',
    }, playback: speech);
  }

  Future<void> replay(String text) =>
      _operate('speak', {'text': text}, playback: true);
  Future<void> startRecording() async {
    if (busy || pendingId != null || recording || _closed) return;
    error = null;
    busy = true;
    phase = 'Starting microphone';
    _transcriptRevision = _draftRevision;
    transcriptForInsertion = null;
    _notify();
    try {
      await audio.startRecording();
      if (_closed) return;
      recording = true;
      phase = 'Recording (maximum 60 seconds)';
    } on Object {
      error = 'Microphone unavailable. Install pulseaudio-utils and check the PulseAudio-compatible session. Typed chat remains available.';
      recording = false;
      _transcriptRevision = null;
      phase = 'Ready';
    } finally {
      busy = false;
    }
    _notify();
  }

  Future<void> transcribe() async {
    if (!recording) return;
    recording = false;
    busy = true;
    phase = 'Stopping recording';
    _notify();
    try {
      final captured = await audio.stopRecording();
      busy = false;
      await _operate('transcribe', {'audio': captured});
    } on Object catch (e) {
      error = e.toString();
      busy = false;
      _notify();
    }
  }

  Future<void> cancelRecording() async {
    recording = false;
    _draftRevision++;
    _transcriptRevision = null;
    phase = 'Ready';
    try {
      await audio.stop();
    } on Object {
      error = 'Audio cleanup could not be confirmed.';
    }
    _notify();
  }

  Future<void> _operate(
    String kind,
    Map<String, dynamic> args, {
    bool playback = false,
  }) async {
    if (busy || pendingId != null || recording || _closed) return;
    if (instanceId == null) {
      error = 'Synchronize service state first.';
      _notify();
      return;
    }
    busy = true;
    error = null;
    phase = kind;
    pendingId = appId();
    pendingKind = kind;
    _expiredRequestId = null;
    _allowPlayback = playback;
    _playbackRevision = _audioRevision;
    if (kind == 'submit') {
      _submittedText = args['text'] as String;
      _submittedRevision = _draftRevision;
    }
    _notify();
    try {
      final reply = await api.call(kind, {'request_id': pendingId, ...args});
      if (_closed) return;
      _correlate(reply);
      if (reply.result['request_id'] != pendingId) {
        throw const AppFailure('PROTOCOL', 'Admission request mismatch.');
      }
      await _poll();
    } on AppFailure catch (e) {
      error = e.toString();
      if (e.code == 'REQUEST_EXPIRED') {
        _expiredRequestId = pendingId;
        _allowPlayback = false;
        phase = 'Request expired · resolution required';
      }
      // A definitive rejected admission is not outstanding work. A transport,
      // malformed acknowledgement or changed instance remains query-only.
      if (!e.unknown &&
          ![
            'PROTOCOL',
            'TRANSPORT',
            'RESTART',
            'REQUEST_EXPIRED',
          ].contains(e.code)) {
        pendingId = null;
        pendingKind = null;
      }
    } on Object {
      error = 'Acknowledgement unknown. Synchronize/query this request; it will not be resent.';
    } finally {
      busy = false;
      _notify();
    }
  }

  Future<void> _poll() async {
    final deadline = DateTime.now().add(operationBudget);
    while (!_closed && pendingId != null) {
      if (DateTime.now().isAfter(deadline)) {
        throw const AppFailure(
          'TIMEOUT',
          'Polling budget elapsed; operation may continue. Synchronize to query, not resend.',
          unknown: true,
        );
      }
      final AppReply reply;
      try {
        reply = await api.call('poll', {
          'request_id': pendingId,
          'instance_id': instanceId,
        });
      } on AppFailure catch (e) {
        if (!_closed && e.code == 'REQUEST_EXPIRED') {
          _expiredRequestId = pendingId;
          _allowPlayback = false;
          phase = 'Request expired · resolution required';
        }
        rethrow;
      }
      if (_closed) return;
      _correlate(reply);
      final job = reply.result;
      if (job['request_id'] != pendingId ||
          job['kind'] != pendingKind ||
          ![
            'running',
            'succeeded',
            'failed',
            'interrupted',
          ].contains(job['status'])) {
        throw const AppFailure(
          'PROTOCOL',
          'Operation correlation/status mismatch.',
        );
      }
      phase = job['phase'] as String;
      _notify();
      if (job['status'] == 'running') {
        await Future<void>.delayed(pollInterval);
        continue;
      }
      final kind = pendingKind;
      final playback = _allowPlayback;
      final playbackRevision = _playbackRevision;
      pendingId = null;
      pendingKind = null;
      _allowPlayback = false;
      if (job['status'] != 'succeeded') {
        throw AppFailure(
          'OPERATION',
          job['error']?.toString() ??
              'Operation interrupted. No automatic retry.',
        );
      }
      final result = Map<String, dynamic>.from(job['result'] as Map);
      if (kind == 'models') {
        models = (result['models'] as List)
            .map((v) => v['name'] as String)
            .toList();
        voices = (result['voices'] as List).cast<String>();
        providers = Map<String, dynamic>.from(result['providers'] as Map);
      } else if (kind == 'transcribe') {
        if (_transcriptRevision != null &&
            _transcriptRevision == _draftRevision) {
          final transcript = result['text'] as String;
          edit(draft.isEmpty ? transcript : '$draft\n$transcript');
        } else {
          transcriptForInsertion = result['text'] as String;
          error =
              'Transcript arrived after editing; draft preserved. Transcript: $transcriptForInsertion';
        }
        _transcriptRevision = null;
      } else if (kind == 'submit') {
        if (_submittedText != null) {
          history = [
            ...history,
            {'role': 'user', 'content': _submittedText},
            {
              'role': 'assistant',
              'content': result['assistant_text'] as String,
            },
          ];
          _submittedText = null;
          if (_submittedRevision == _draftRevision) edit('');
          _submittedRevision = null;
          _notify();
        }
        // Synchronize authoritative bounded history, never append/replay an
        // uncertain turn locally or autoplay audio restored from service state.
        final state = await api.call('state', {});
        if (_closed) return;
        _correlate(state);
        _readState(state.result);
        _notify();
        if (result['speech_error'] != null) {
          error = 'Answer saved; speech unavailable: ${result['speech_error']}';
        }
      }
      if (!_closed &&
          playback &&
          playbackRevision == _audioRevision &&
          (speech || kind == 'speak') &&
          result['audio'] is Map) {
        final revision = _audioRevision;
        playing = true;
        phase = 'Playing';
        _notify();
        try {
          if (revision == _audioRevision) {
            await audio.play(Map<String, dynamic>.from(result['audio'] as Map));
          }
        } on Object {
          error = 'Answer retained; playback unavailable.';
        } finally {
          playing = false;
        }
      }
      phase = 'Ready';
      _notify();
      return;
    }
  }

  @override
  void dispose() {
    _closed = true;
    _audioRevision++;
    api.dispose();
    audio.dispose();
    super.dispose();
  }
}

class ChatPanel extends StatefulWidget {
  const ChatPanel({
    super.key,
    required this.controller,
    this.settings = false,
    this.services = false,
  });
  final ChatController controller;
  final bool settings;
  final bool services;
  @override
  State<ChatPanel> createState() => _ChatPanelState();
}

class _ChatPanelState extends State<ChatPanel> {
  late final TextEditingController _draft = TextEditingController(
    text: widget.controller.draft,
  );
  String? _model;
  String? _voice;
  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_sync);
  }

  void _sync() {
    final value = widget.controller.draft;
    if (_draft.text != value) {
      _draft.value = TextEditingValue(
        text: value,
        selection: TextSelection.collapsed(offset: value.length),
      );
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_sync);
    _draft.dispose();
    super.dispose();
  }

  Future<void> _resolveExpiredRequest() async {
    final c = widget.controller;
    final id = c.pendingId;
    if (id == null || !c.canAbandonExpiredRequest) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Forget expired request?'),
        content: const Text(
          'The service no longer retains this request result. Its outcome may be unknown. Review saved history before sending new work to avoid duplicate work. This only releases the local pending request; it does not resend or cancel service work.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context, false),
            child: const Text('Keep pending'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, true),
            child: const Text('Forget request'),
          ),
        ],
      ),
    );
    if (mounted && confirmed == true) c.abandonExpiredRequest(id);
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
    listenable: widget.controller,
    builder: (context, _) {
      final c = widget.controller;
      final idle = !c.busy && c.pendingId == null && !c.recording;
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            'Local chat · ${c.phase}',
            style: Theme.of(context).textTheme.titleLarge,
          ),
          Text(
            'Selected model: ${c.config['model'] ?? 'none'} · voice: ${c.config['voice'] ?? 'none'}',
          ),
          if (c.error != null) SelectableText(c.error!),
          Wrap(
            spacing: 8,
            children: [
              TextButton(
                onPressed: c.busy ? null : c.synchronize,
                child: const Text('Synchronize chat'),
              ),
              if (widget.settings)
                TextButton(
                  onPressed: idle ? c.refreshModels : null,
                  child: const Text('Refresh models and speech status'),
                ),
            ],
          ),
          if (c.pendingId != null)
            SelectableText('Request ${c.pendingId} · no automatic resend'),
          if (c.canAbandonExpiredRequest)
            TextButton(
              onPressed: c.busy ? null : _resolveExpiredRequest,
              child: const Text('Resolve expired request'),
            ),
          if (widget.settings) ...[
            SelectableText('Observed providers: ${c.providers}'),
            DropdownButton<String>(
              value: c.models.contains(_model ?? c.config['model'])
                  ? (_model ?? c.config['model']) as String?
                  : null,
              hint: const Text('Select installed local model'),
              isExpanded: true,
              items: c.models
                  .map((v) => DropdownMenuItem(value: v, child: Text(v)))
                  .toList(),
              onChanged: idle ? (v) => setState(() => _model = v) : null,
            ),
            DropdownButton<String>(
              value: c.voices.contains(_voice ?? c.config['voice'])
                  ? (_voice ?? c.config['voice']) as String?
                  : null,
              hint: const Text('No speech voice selected'),
              isExpanded: true,
              items: [
                const DropdownMenuItem<String>(
                  value: '',
                  child: Text('No speech voice'),
                ),
                ...c.voices.map(
                  (v) => DropdownMenuItem(value: v, child: Text(v)),
                ),
              ],
              onChanged: idle ? (v) => setState(() => _voice = v) : null,
            ),
            ElevatedButton(
              onPressed: idle && (_model ?? c.config['model']) != null
                  ? () => c.configure(
                      _model ?? c.config['model'] as String?,
                      (_voice ?? c.config['voice']) == ''
                          ? null
                          : (_voice ?? c.config['voice']) as String?,
                    )
                  : null,
              child: const Text('Save model and voice'),
            ),
            const Text(
              'Refresh observes already-running local providers only. No start, download, cloud or fallback.',
            ),
            if (widget.services)
              const Text(
                'Speech uses mounted local Whisper/Kokoro models. Search remains planned; no search requests are available.',
              ),
          ] else ...[
            for (final message in c.history)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 8),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      message['role'] == 'user' ? 'You' : 'Smith',
                      style: Theme.of(context).textTheme.labelLarge,
                    ),
                    SelectableText(message['content'] as String),
                    if (message['role'] == 'assistant')
                      TextButton(
                        onPressed: idle
                            ? () => c.replay(message['content'] as String)
                            : null,
                        child: const Text('Speak this answer'),
                      ),
                  ],
                ),
              ),
            Wrap(
              spacing: 8,
              children: [
                ChoiceChip(
                  label: const Text('Type'),
                  selected: !c.microphone,
                  onSelected: c.recording
                      ? null
                      : (_) {
                          c.microphone = false;
                          setState(() {});
                        },
                ),
                ChoiceChip(
                  label: const Text('Microphone'),
                  selected: c.microphone,
                  onSelected: c.recording
                      ? null
                      : (_) {
                          c.microphone = true;
                          setState(() {});
                        },
                ),
                ChoiceChip(
                  label: const Text('Text'),
                  selected: !c.speech,
                  onSelected: (_) => c.setSpeech(false),
                ),
                ChoiceChip(
                  label: const Text('Text + speech'),
                  selected: c.speech,
                  onSelected: (_) => c.setSpeech(true),
                ),
              ],
            ),
            const SizedBox(height: 12),
            TextField(
              controller: _draft,
              onChanged: c.edit,
              minLines: 2,
              maxLines: 6,
              decoration: const InputDecoration(
                labelText: 'Editable draft',
                hintText: 'Type or record, review, then Send',
                border: OutlineInputBorder(),
              ),
            ),
            if (c.transcriptForInsertion != null) ...[
              const Text('Transcript available · draft preserved'),
              SelectableText(c.transcriptForInsertion!),
              TextButton(
                onPressed: idle ? c.insertTranscript : null,
                child: const Text('Insert transcript into draft'),
              ),
            ],
            Wrap(
              spacing: 8,
              children: [
                if (c.microphone && !c.recording)
                  TextButton(
                    onPressed: idle ? c.startRecording : null,
                    child: const Text('Record'),
                  ),
                if (c.recording) ...[
                  TextButton(
                    onPressed: c.transcribe,
                    child: const Text('Stop and transcribe'),
                  ),
                  TextButton(
                    onPressed: c.cancelRecording,
                    child: const Text('Cancel recording'),
                  ),
                ],
                ElevatedButton(
                  onPressed: idle && c.draft.trim().isNotEmpty ? c.send : null,
                  child: const Text('Send'),
                ),
                if (c.playing)
                  TextButton(
                    onPressed: () => c.setSpeech(false),
                    child: const Text('Stop audio'),
                  ),
              ],
            ),
            const Text(
              'Text is always retained. Speech failures do not repeat generation. Closing detaches; service work may continue.',
            ),
          ],
        ],
      );
    },
  );
}
