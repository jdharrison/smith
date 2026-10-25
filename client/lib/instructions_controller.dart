import 'package:flutter/foundation.dart';

import 'app_api.dart';
import 'app_v2.dart' as v2;
import 'cloud_routing.dart';
import 'instructions.dart';

/// Exact local review, including the editor value rather than just its revision.
final class InstructionReview {
  const InstructionReview._(
    this.instanceId,
    this.configRevision,
    this.settings,
    this._owner,
    this._edit,
  );
  final String instanceId;
  final Object configRevision;
  final InstructionSettings settings;
  final InstructionsController _owner;
  final int _edit;
}

/// Desktop-owned volatile editor; disposing this never disposes the shared API.
class InstructionsController extends ChangeNotifier {
  InstructionsController(this._api, {this.onApplied});
  final AppApi _api;
  final Future<void> Function()? onApplied;
  InstructionSnapshot? _snapshot;
  String? _instanceId, _draftInstanceId, _error;
  Object? _draftRevision;
  InstructionSettings _draft = const InstructionSettings();
  InstructionSettings? _base;
  bool _closed = false, _loading = false, _busy = false, _fresh = false;
  int _edit = 0;
  int _notificationDepth = 0;
  ChatRoute? _route, _snapshotRoute;
  _PendingInstructions? _pending;

  InstructionSnapshot? get snapshot => _snapshot;
  ChatRoute? get snapshotRoute => _snapshotRoute;
  String? get instanceId => _instanceId;
  Object? get configRevision => _snapshot?.configRevision;
  String? get draftInstanceId => _draftInstanceId;
  Object? get draftConfigRevision => _draftRevision;
  bool get loading => _loading;
  bool get busy => _busy;
  bool get uncertain => _pending != null;
  String? get pendingRequestId => _pending?.requestId;
  String? get error => _error;
  InstructionSettings get draft => _draft;
  bool get dirty => _base != null && _draft != _base;
  bool get conflict =>
      _base != null &&
      (_draftInstanceId != _instanceId || _draftRevision != configRevision);
  bool get canApply =>
      !_closed &&
      _fresh &&
      !_loading &&
      !_busy &&
      !uncertain &&
      !conflict &&
      dirty &&
      _draft.validationError == null &&
      instructionRevisionNumber(configRevision!) <
          BigInt.parse('18446744073709551615');

  void edit(InstructionSettings value) {
    if (_closed) return;
    _base ??= _snapshot?.settings ?? const InstructionSettings();
    _draft = value;
    _edit++;
    _emit();
  }

  /// Explicitly abandon edits and bind to the last successfully observed state.
  void discard() {
    if (_closed ||
        _busy ||
        _loading ||
        uncertain ||
        !_fresh ||
        _snapshot == null) {
      return;
    }
    _resetDraft();
    _emit();
  }

  /// Call only for an explicit operator Review. Refresh never rebinds dirty edits.
  InstructionReview? createReview() {
    if (_closed ||
        !_fresh ||
        _busy ||
        _loading ||
        uncertain ||
        _snapshot == null ||
        _draft.validationError != null) {
      return null;
    }
    _base = _snapshot!.settings;
    _draftInstanceId = _instanceId;
    _draftRevision = configRevision;
    final review = InstructionReview._(
      _instanceId!,
      configRevision!,
      _draft,
      this,
      _edit,
    );
    _emit();
    return review;
  }

  Future<void> refresh({ChatRoute? route}) async {
    if (_closed || _busy || _loading) return;
    _loading = true;
    _fresh = false;
    _error = null;
    _route = route;
    _emit();
    try {
      await _read(route);
      if (!_closed && uncertain) _error = _uncertainMessage;
    } on Object catch (e) {
      if (!_closed) _error = _message(e);
    } finally {
      if (!_closed) {
        _loading = false;
        _emit();
      }
    }
  }

  Future<void> apply({required InstructionReview review}) async {
    if (_closed) return;
    if (!identical(review._owner, this) ||
        review._edit != _edit ||
        review.settings != _draft ||
        review.instanceId != _instanceId ||
        review.configRevision != configRevision ||
        !canApply) {
      _error = 'Review is stale or Apply is unavailable. Refresh if needed, then explicitly review the preserved draft again.';
      _emit();
      return;
    }
    final pending = _PendingInstructions(
      appId(),
      review,
      _edit,
      _snapshot!.settings,
    );
    _pending = pending;
    _busy = true;
    _fresh = false;
    _error = null;
    _emit();
    var acknowledged = false;
    try {
      if (_closed) return;
      final args = <String, dynamic>{
        'request_id': pending.requestId,
        'instance_id': review.instanceId,
        'expected_config_revision': review.configRevision,
        'settings': review.settings.toJson(),
      };
      final reply = await _api.call('instructions_update', args);
      if (_closed) return;
      if (reply.instanceId != review.instanceId) {
        throw const AppFailure(
          'PROTOCOL',
          'Instruction acknowledgement instance mismatch.',
        );
      }
      _acceptReceipt(pending, reply);
      acknowledged = true;
      await _read(_route, minimum: pending.receipt!.configRevision);
      if (_closed) return;
      _reconcile(pending);
    } on Object catch (e) {
      if (!_closed) {
        if (!acknowledged && _definite(e)) {
          _pending = null;
          _error = _message(e);
        } else {
          _error = _uncertainMessage;
        }
      }
    } finally {
      if (!_closed) {
        _busy = false;
        _emit();
      }
    }
    if (!_closed && _pending == null && acknowledged) await _applied();
  }

  /// Observation only, always using the original identity, never a replacement write.
  Future<void> resolvePending() async {
    final pending = _pending;
    if (_closed || _busy || _loading || pending == null) return;
    _busy = true;
    _fresh = false;
    _error = null;
    _emit();
    var resolved = false;
    try {
      if (_closed) return;
      final reply = await _api.call('instructions_receipt', {
        'request_id': pending.requestId,
      });
      if (_closed) return;
      _acceptReceipt(pending, reply);
      await _read(_route, minimum: pending.receipt!.configRevision);
      if (_closed) return;
      _reconcile(pending);
      resolved = true;
    } on Object catch (e) {
      if (!_closed) {
        _error =
            e is AppFailure &&
                const {'REQUEST_UNKNOWN', 'REQUEST_EXPIRED'}.contains(e.code)
            ? 'The original instruction receipt is unknown or expired. Its outcome remains uncertain; writes are blocked. Nothing was resent.'
            : _uncertainMessage;
        final message = _error;
        // Current settings are useful observation, never proof of an unknown write.
        try {
          await _read(_route, minimum: pending.receipt?.configRevision);
        } on Object {
          /* Keep the original uncertainty. */
        }
        if (!_closed) _error = message;
      }
    } finally {
      if (!_closed) {
        _busy = false;
        _emit();
      }
    }
    if (!_closed && resolved) await _applied();
  }

  Future<void> _read(ChatRoute? route, {Object? minimum}) async {
    if (_closed) return;
    final reply = await _api.call('instructions_get', {
      'route': route?.toJson(),
    });
    if (_closed) return;
    _validateInstance(reply.instanceId);
    final next = InstructionSnapshot.parse(reply.result)..matchRoute(route);
    for (final floor in [configRevision, minimum]) {
      if (floor != null &&
          instructionRevisionNumber(next.configRevision) <
              instructionRevisionNumber(floor)) {
        throw const AppFailure('PROTOCOL', 'Stale instruction snapshot.');
      }
    }
    final preserve = _base != null && (dirty || uncertain || conflict);
    _snapshot = next;
    _snapshotRoute = route;
    _instanceId = reply.instanceId;
    _fresh = true;
    if (!preserve) _resetDraft();
  }

  void _resetDraft() {
    _draft = _base = _snapshot!.settings;
    _draftInstanceId = _instanceId;
    _draftRevision = configRevision;
    _edit++;
  }

  void _acceptReceipt(_PendingInstructions pending, AppReply reply) {
    _validateInstance(reply.instanceId);
    correlateInstructionResult('instructions_update', {
      'request_id': pending.requestId,
      'expected_config_revision': pending.review.configRevision,
    }, reply.result);
    final receipt = InstructionReceipt.parse(reply.result);
    if (receipt.changed != (pending.before != pending.review.settings)) {
      throw const AppFailure(
        'PROTOCOL',
        'Instruction receipt change mismatch.',
      );
    }
    if (pending.receipt != null &&
        (pending.receipt!.configRevision != receipt.configRevision ||
            pending.receipt!.changed != receipt.changed)) {
      throw const AppFailure('PROTOCOL', 'Instruction receipt changed.');
    }
    pending.receipt = receipt;
  }

  void _reconcile(_PendingInstructions pending) {
    if (configRevision == pending.receipt!.configRevision &&
        _snapshot!.settings != pending.review.settings) {
      throw const AppFailure(
        'PROTOCOL',
        'Instruction settings do not match the acknowledged revision.',
      );
    }
    // A later authoritative value wins; newer typing stays bound to its old base.
    if (_edit == pending.edit) {
      _resetDraft();
    } else {
      _base = _snapshot!.settings;
    }
    _pending = null;
    _error = null;
  }

  Future<void> _applied() async {
    try {
      await onApplied?.call();
    } on Object {
      if (!_closed) {
        _error = 'Instructions were applied, but chat synchronization failed. Synchronize chats before sending.';
        _emit();
      }
    }
  }

  static const _uncertainMessage =
      'Instruction Apply outcome is uncertain. Resolve the original receipt and observe current settings before another write. Nothing will be resent.';
  bool _definite(Object e) =>
      e is AppFailure &&
      !e.unknown &&
      const {
        'BUSY',
        'STALE',
        'STALE_REVISION',
        'REVISION_MISMATCH',
        'INSTANCE_MISMATCH',
        'REVISION_EXHAUSTED',
        'CONFIG_REVISION_EXHAUSTED',
        'CAPACITY',
        'LIMIT',
        'INVALID_REQUEST',
        'INVALID_ARGUMENT',
        'ARGUMENT',
        'ID_CONFLICT',
        'REQUEST_EXPIRED',
        'OP',
        'UPGRADE_REQUIRED',
        'CLOSED',
      }.contains(e.code);
  String _message(Object e) => switch (e) {
    AppFailure(code: 'BUSY') => 'Smith has accepted work queued or running. Wait for it to finish, refresh and review your preserved draft; Apply was not sent again.',
    AppFailure(
      code: 'STALE' ||
          'STALE_REVISION' ||
          'REVISION_MISMATCH' ||
          'INSTANCE_MISMATCH',
    ) =>
      'Smith or its configuration changed. Refresh and explicitly review the preserved draft before Apply.',
    AppFailure(code: 'OP' || 'UPGRADE_REQUIRED') => 'This service does not support shared instructions. Use a compatible paired service; no fallback or write was attempted.',
    AppFailure(code: 'PROTOCOL') => 'Shared instruction data could not be validated. Refresh or resolve the original receipt; edits are preserved.',
    _ => 'Shared instructions are unavailable. Refresh or resolve the original receipt; edits are preserved and nothing will be resent.',
  };
  void _validateInstance(String value) {
    if (value.length != 32 || !v2.wireId(value)) {
      throw const AppFailure('PROTOCOL', 'Invalid service instance.');
    }
  }

  void _emit() {
    if (!_closed) notifyListeners();
  }

  @override
  void notifyListeners() {
    if (_closed) return;
    _notificationDepth++;
    try {
      super.notifyListeners();
    } finally {
      _notificationDepth--;
      if (_closed && _notificationDepth == 0) super.dispose();
    }
  }

  @override
  void dispose() {
    if (_closed) return;
    _closed = true;
    if (_notificationDepth == 0) super.dispose();
  }
}

final class _PendingInstructions {
  _PendingInstructions(this.requestId, this.review, this.edit, this.before);
  final InstructionSettings before;
  final String requestId;
  final InstructionReview review;
  final int edit;
  InstructionReceipt? receipt;
}
