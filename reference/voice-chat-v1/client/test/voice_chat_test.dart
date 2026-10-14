import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:smith_client/app_api.dart';
import 'package:smith_client/audio.dart';
import 'package:smith_client/voice_chat.dart';

import 'app_fakes.dart';

Future<void> tick() async {
  for (var i = 0; i < 10; i++) {
    await Future<void>.delayed(Duration.zero);
  }
}

void main() {
  test(
    'dictation appends to a typed prefix without automatic submission',
    () async {
      final api = FakeAppApi();
      final c = ChatController(api, FakeAudio());
      await c.synchronize();
      c.edit('typed prefix');
      await c.startRecording();
      await c.transcribe();
      expect(c.draft, 'typed prefix\ntranscript to review');
      expect(c.transcriptForInsertion, isNull);
      expect(api.calls, ['state', 'transcribe', 'poll']);
      c.dispose();
    },
  );

  test('repeated dictation appends and subsequent cancellation preserves the draft', () async {
    final api = FakeAppApi();
    final c = ChatController(api, FakeAudio());
    await c.synchronize();
    c.edit('typed prefix');
    await c.startRecording();
    await c.transcribe();
    await c.startRecording();
    await c.transcribe();
    const combined = 'typed prefix\ntranscript to review\ntranscript to review';
    expect(c.draft, combined);
    expect(api.calls.where((op) => op == 'transcribe').length, 2);
    final callsBeforeCancel = List<String>.from(api.calls);
    await c.startRecording();
    await c.cancelRecording();
    expect(c.draft, combined);
    expect(c.recording, isFalse);
    expect(api.calls, callsBeforeCancel);
    expect(api.calls, isNot(contains('submit')));
    c.dispose();
  });

  test(
    'M1 edits during recording survive transcription until explicit insertion',
    () async {
      final api = FakeAppApi();
      final c = ChatController(api, FakeAudio());
      await c.synchronize();
      c.edit('before capture');
      await c.startRecording();
      c.edit('edited while recording');
      await c.transcribe();
      expect(c.draft, 'edited while recording');
      expect(c.transcriptForInsertion, 'transcript to review');
      expect(api.calls, isNot(contains('submit')));
      final input =
          api.arguments[api.calls.indexOf('transcribe')]['audio'] as Map;
      expect(input.keys, unorderedEquals(['mime', 'data_base64']));
      c.insertTranscript();
      expect(c.draft, 'edited while recording\ntranscript to review');
      expect(c.transcriptForInsertion, isNull);
      expect(api.calls, isNot(contains('submit')));
      c.dispose();
    },
  );

  test('M2 speech-off does not stop microphone capture', () async {
    final api = FakeAppApi();
    final audio = FakeAudio();
    final c = ChatController(api, audio);
    await c.synchronize();
    c.setSpeech(true);
    await c.startRecording();
    final stops = audio.stops;
    c.setSpeech(false);
    expect(audio.stops, stops);
    expect(c.recording, isTrue);
    expect(c.phase, 'Recording (maximum 60 seconds)');
    await c.transcribe();
    expect(c.recording, isFalse);
    expect(c.draft, 'transcript to review');
    expect(audio.stops, stops + 1);
    expect(audio.plays, 0);
    c.dispose();
  });

  test('M2 speech-off still stops active playback', () async {
    final api = FakeAppApi();
    final audio = FakeAudio()..playback = Completer<void>();
    final c = ChatController(api, audio);
    await c.synchronize();
    c.setSpeech(true);
    c.edit('hello');
    final work = c.send();
    await tick();
    expect(c.playing, isTrue);
    c.setSpeech(false);
    await work;
    expect(audio.stops, 1);
    expect(c.playing, isFalse);
    expect(c.history.last['content'], 'useful answer');
    c.dispose();
  });

  test('M3 expired unknown acknowledgement requires explicit local resolution, never resend', () async {
    final api = FakeAppApi()..unknownAdmission = true;
    final c = ChatController(api, FakeAudio());
    await c.synchronize();
    c.edit('uncertain work');
    await c.send();
    final oldId = c.pendingId!;
    c.abandonExpiredRequest(oldId);
    expect(c.pendingId, oldId);
    api.expiredPoll = true;
    await c.synchronize();
    expect(c.pendingId, oldId);
    expect(c.canAbandonExpiredRequest, isTrue);
    expect(c.error, contains('REQUEST_EXPIRED'));
    await c.send();
    await c.configure('local:exact', null);
    expect(api.calls.where((v) => v == 'submit').length, 1);
    expect(api.calls, isNot(contains('configure')));
    c.abandonExpiredRequest('ffffffffffffffffffffffffffffffff');
    expect(c.pendingId, oldId);
    c.abandonExpiredRequest(oldId);
    expect(c.pendingId, isNull);
    expect(c.canAbandonExpiredRequest, isFalse);
    expect(c.phase, 'Ready');
    expect(c.draft, 'uncertain work');
    expect(c.error, contains('outcome may be unknown'));
    expect(c.history.last['content'], 'useful answer');
    expect(api.calls.where((v) => v == 'submit').length, 1);
    api.unknownAdmission = false;
    api.expiredPoll = false;
    await c.refreshModels();
    expect(c.models, ['local:exact']);
    c.edit('deliberately new work');
    await c.send();
    final submissions = [
      for (var i = 0; i < api.calls.length; i++)
        if (api.calls[i] == 'submit') api.arguments[i],
    ];
    expect(submissions.length, 2);
    expect(submissions.last['request_id'], isNot(oldId));
    expect(submissions.last['text'], 'deliberately new work');
    c.dispose();
  });

  test('M4 microphone failure is visible and returns idle without blocking typed chat', () async {
    final api = FakeAppApi();
    final c = ChatController(api, FakeAudio()..unavailable = true);
    await c.synchronize();
    c.edit('keep this draft');
    await c.startRecording();
    expect(c.error, contains('Microphone unavailable'));
    expect(c.error, contains('Typed chat remains available'));
    expect(c.phase, 'Ready');
    expect(c.busy, isFalse);
    expect(c.recording, isFalse);
    expect(c.draft, 'keep this draft');
    expect(api.calls, ['state']);
    await c.send();
    expect(c.history.first['content'], 'keep this draft');
    c.dispose();
  });

  testWidgets('M1 transcript insertion is explicit and does not send', (
    tester,
  ) async {
    final api = FakeAppApi();
    final c = ChatController(api, FakeAudio());
    await c.synchronize();
    c.edit('prior');
    await c.startRecording();
    c.edit('edited during capture');
    await c.transcribe();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(child: ChatPanel(controller: c)),
        ),
      ),
    );
    expect(find.text('Transcript available · draft preserved'), findsOneWidget);
    expect(find.text('transcript to review'), findsOneWidget);
    await tester.ensureVisible(find.text('Insert transcript into draft'));
    await tester.tap(find.text('Insert transcript into draft'));
    await tester.pumpAndSettle();
    expect(c.draft, 'edited during capture\ntranscript to review');
    expect(find.text('Insert transcript into draft'), findsNothing);
    expect(api.calls, isNot(contains('submit')));
    await tester.pumpWidget(const SizedBox());
    c.dispose();
  });

  testWidgets(
    'M3 expired resolution requires confirmation and never resubmits',
    (tester) async {
      final api = FakeAppApi()..unknownAdmission = true;
      final c = ChatController(api, FakeAudio());
      await c.synchronize();
      c.edit('uncertain draft');
      await c.send();
      api.expiredPoll = true;
      await c.synchronize();
      final id = c.pendingId;
      await tester.pumpWidget(
        MaterialApp(
          home: Scaffold(
            body: SingleChildScrollView(child: ChatPanel(controller: c)),
          ),
        ),
      );
      await tester.tap(find.text('Resolve expired request'));
      await tester.pumpAndSettle();
      expect(find.textContaining('outcome may be unknown'), findsOneWidget);
      await tester.tap(find.text('Keep pending'));
      await tester.pumpAndSettle();
      expect(c.pendingId, id);
      await tester.tap(find.text('Resolve expired request'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Forget request'));
      await tester.pumpAndSettle();
      expect(c.pendingId, isNull);
      expect(find.text('Resolve expired request'), findsNothing);
      expect(api.calls.where((v) => v == 'submit').length, 1);
      expect(api.calls, isNot(contains('cancel')));
      await tester.pumpWidget(const SizedBox());
      c.dispose();
    },
  );

  testWidgets('M4 microphone outage error renders with usable Send', (
    tester,
  ) async {
    final api = FakeAppApi();
    final c = ChatController(api, FakeAudio()..unavailable = true);
    await c.synchronize();
    c.edit('typed draft');
    await c.startRecording();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(child: ChatPanel(controller: c)),
        ),
      ),
    );
    expect(find.textContaining('Microphone unavailable'), findsOneWidget);
    expect(find.text('Local chat · Ready'), findsOneWidget);
    expect(
      tester
          .widget<ElevatedButton>(find.widgetWithText(ElevatedButton, 'Send'))
          .onPressed,
      isNotNull,
    );
    await tester.pumpWidget(const SizedBox());
    c.dispose();
  });

  for (final mic in [false, true]) {
    for (final speech in [false, true]) {
      test(
        '${mic ? 'Mic' : 'Type'} to ${speech ? 'Text+speech' : 'Text'} requires explicit send',
        () async {
          final api = FakeAppApi();
          final audio = FakeAudio();
          final c = ChatController(api, audio);
          await c.synchronize();
          c.setSpeech(speech);
          if (mic) {
            await c.startRecording();
            await c.transcribe();
            expect(c.draft, 'transcript to review');
            expect(api.calls, isNot(contains('submit')));
            c.edit('reviewed transcript');
          } else {
            c.edit('typed request');
          }
          await c.send();
          expect(api.calls.where((v) => v == 'submit').length, 1);
          expect(
            api.arguments[api.calls.indexOf('submit')]['output'],
            speech ? 'speech' : 'chat',
          );
          expect(audio.plays, speech ? 1 : 0);
          expect(c.history.last['content'], 'useful answer');
          expect(c.draft, '');
          c.dispose();
        },
      );
    }
  }
  test('initial attach reads state only; inventory explicit and configure persists', () async {
    final api = FakeAppApi();
    final audio = FakeAudio();
    final c = ChatController(api, audio);
    await c.synchronize();
    expect(api.calls, ['state']);
    expect(audio.plays, 0);
    await c.refreshModels();
    expect(c.models, ['local:exact']);
    expect(c.voices, ['observed_voice']);
    await c.configure('local:exact', 'observed_voice');
    expect(c.config['voice'], 'observed_voice');
    expect(c.config['revision'], 2);
    c.dispose();
  });
  test(
    'missing speech cannot block typed text, failed TTS retains answer',
    () async {
      final api = FakeAppApi()..speechFailure = true;
      final audio = FakeAudio()..unavailable = true;
      final c = ChatController(api, audio);
      await c.synchronize();
      await c.startRecording();
      expect(c.recording, false);
      c.setSpeech(true);
      c.edit('type instead');
      await c.send();
      expect(c.history.last['content'], 'useful answer');
      expect(c.error, contains('speech unavailable'));
      expect(audio.plays, 0);
      expect(api.calls.where((v) => v == 'submit').length, 1);
      c.dispose();
    },
  );
  test(
    'editing during transcription preserves draft and shows late transcript',
    () async {
      final api = FakeAppApi();
      final c = ChatController(api, FakeAudio());
      await c.synchronize();
      c.edit('prior');
      await c.startRecording();
      api.pollGate = Completer<AppReply>();
      final work = c.transcribe();
      await tick();
      c.edit('new editing');
      api.pollGate!.complete(AppReply(testInstance, api.jobs.values.single));
      await work;
      expect(c.draft, 'new editing');
      expect(c.error, contains('transcript to review'));
      expect(api.calls, isNot(contains('submit')));
      c.dispose();
    },
  );
  test('cancel recording preserves earlier draft', () async {
    final api = FakeAppApi();
    final c = ChatController(api, FakeAudio());
    await c.synchronize();
    c.edit('prior');
    await c.startRecording();
    await c.cancelRecording();
    expect(c.draft, 'prior');
    expect(api.calls, ['state']);
    c.dispose();
  });
  test(
    'speech off then on suppresses pending playback and never repeats LLM',
    () async {
      final api = FakeAppApi();
      final audio = FakeAudio();
      final c = ChatController(api, audio);
      await c.synchronize();
      c.setSpeech(true);
      c.edit('hello');
      api.pollGate = Completer<AppReply>();
      final work = c.send();
      await tick();
      c.setSpeech(false);
      c.setSpeech(true);
      api.pollGate!.complete(AppReply(testInstance, api.jobs.values.single));
      await work;
      expect(audio.plays, 0);
      expect(c.history.last['content'], 'useful answer');
      expect(api.calls.where((v) => v == 'submit').length, 1);
      c.dispose();
    },
  );
  test(
    'unknown admission is queried, never resent, reattach never plays',
    () async {
      final api = FakeAppApi()..unknownAdmission = true;
      final audio = FakeAudio();
      final c = ChatController(api, audio);
      await c.synchronize();
      c.setSpeech(true);
      c.edit('hello');
      await c.send();
      final id = c.pendingId;
      expect(id, matches(RegExp(r'^[0-9a-f]{32}$')));
      await c.send();
      expect(api.calls.where((v) => v == 'submit').length, 1);
      c.setSpeech(false);
      await c.synchronize();
      expect(c.pendingId, isNull);
      expect(api.arguments[api.calls.indexOf('poll')]['request_id'], id);
      expect(audio.plays, 0);
      c.dispose();
      final reopenedAudio = FakeAudio();
      final reopened = ChatController(api, reopenedAudio);
      await reopened.synchronize();
      expect(reopened.history.last['content'], 'useful answer');
      expect(reopenedAudio.plays, 0);
      reopened.dispose();
    },
  );
  test('restart does not resend unknown work', () async {
    final api = FakeAppApi()..unknownAdmission = true;
    final c = ChatController(api, FakeAudio());
    await c.synchronize();
    c.edit('hello');
    await c.send();
    api.instance = 'ffffffffffffffffffffffffffffffff';
    await c.synchronize();
    expect(c.error, contains('restarted'));
    expect(api.calls.where((v) => v == 'submit').length, 1);
    c.dispose();
  });
  test('poll deadline is finite and retains query-only operation', () async {
    final api = FakeAppApi();
    final c = ChatController(
      api,
      FakeAudio(),
      operationBudget: const Duration(milliseconds: 5),
      pollInterval: const Duration(milliseconds: 10),
    );
    await c.synchronize();
    api.pollGate = Completer<AppReply>();
    c.edit('hello');
    final work = c.send();
    await tick();
    final job = {
      ...api.jobs.values.single,
      'status': 'running',
      'phase': 'generation',
    };
    api.pollGate!.complete(AppReply(testInstance, job));
    await work;
    expect(c.pendingId, isNotNull);
    expect(c.error, contains('Polling budget'));
    c.dispose();
  });
  test(
    'deliberate replay only calls speak and audio cleanup on dispose',
    () async {
      final api = FakeAppApi();
      final audio = FakeAudio();
      final c = ChatController(api, audio);
      await c.synchronize();
      await c.replay('answer');
      expect(api.calls, ['state', 'speak', 'poll']);
      expect(audio.plays, 1);
      c.dispose();
      expect(audio.disposed, true);
      expect(api.disposed, true);
    },
  );
  test('strict audio rejects oversized malformed rate stereo and duration', () {
    expect(validateWav(testAudio(24000), 24000).length, 4);
    expect(() => validateWav(testAudio(16000), 24000), throwsFormatException);
    final bytes = wavPcm([0, 0], 24000);
    bytes[22] = 2;
    expect(
      () => validateWav({
        'mime': 'audio/wav',
        'data_base64': base64Encode(bytes),
      }, 24000),
      throwsFormatException,
    );
    expect(
      () => validateWav({
        'mime': 'audio/wav',
        'data_base64': 'A' * 4194305,
      }, 24000),
      throwsFormatException,
    );
  });
  testWidgets('composer exposes all modes editable draft and explicit Send', (
    tester,
  ) async {
    final api = FakeAppApi();
    final c = ChatController(api, FakeAudio());
    await c.synchronize();
    await tester.pumpWidget(
      MaterialApp(
        home: Scaffold(
          body: SingleChildScrollView(child: ChatPanel(controller: c)),
        ),
      ),
    );
    for (final label in [
      'Type',
      'Microphone',
      'Text',
      'Text + speech',
      'Send',
    ]) {
      expect(find.text(label), findsOneWidget);
    }
    await tester.enterText(find.byType(TextField), 'edited');
    await tester.pump();
    expect(c.draft, 'edited');
    expect(api.calls, ['state']);
    await tester.tap(find.text('Send'));
    await tester.pumpAndSettle();
    expect(find.text('useful answer'), findsOneWidget);
    await tester.pumpWidget(const SizedBox());
    c.dispose();
  });
}
