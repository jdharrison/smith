import 'dart:async';
import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:math';
import 'dart:typed_data';

import 'workspace_preferences.dart';

const _limit = 4096;
const _directory = 0x10000;
const _noFollow = 0x20000;
const _closeExec = 0x80000;
const _nonBlock = 0x800;
const _create = 0x40;
const _exclusive = 0x80;
const _fileName = 'workspace.json';

/// Linux x86_64 only (the supported workstation ABI).
///
/// [root] is an existing, owner-controlled state root. Only smith-next/client
/// beneath it is created. Environment construction may create missing fallback
/// directories under HOME. No filesystem or libc access occurs at construction.
/// Atomic publication is provided, not directory-fsync/power-loss durability.
class LinuxWorkspacePreferencesStore implements WorkspacePreferencesStore {
  factory LinuxWorkspacePreferencesStore({required String root}) =>
      LinuxWorkspacePreferencesStore._(root, null);

  LinuxWorkspacePreferencesStore._(this._root, this._home);

  factory LinuxWorkspacePreferencesStore.fromEnvironment() {
    final state = Platform.environment['XDG_STATE_HOME'];
    return LinuxWorkspacePreferencesStore._(
      state == null || state.isEmpty ? null : state,
      Platform.environment['HOME'],
    );
  }

  final String? _root;
  final String? _home;
  _LibC? _native;
  int _dir = -1;
  int _lock = -1;
  bool _closed = false;
  bool _blocked = false;
  Future<WorkspacePreferences?>? _loading;
  Future<void>? _saving;
  Future<void>? _closing;
  WorkspacePreferences? _pending;
  _Snapshot? _baseline;

  @override
  Future<WorkspacePreferences?> load() {
    if (_closed || _blocked) return Future.error(_error());
    return _loading ??= Future.sync(() {
      var stage = 'platform';
      try {
        if (_closed) throw _error();
        if (!Platform.isLinux || Abi.current() != Abi.linuxX64) {
          throw _error();
        }
        _native = _LibC();
        stage = 'directory';
        _openDirectory();
        final c = _native!;
        stage = 'ownership';
        _lock = c.open(_dir, '.workspace.lock', 2 | _create, 0x180);
        c.privateFile(_lock);
        // flock is bound to the independently opened file description, unlike
        // POSIX record locks: competing instances in this process also conflict.
        if (c.flock(_lock, 2 | 4) != 0) throw _error();
        _verifyLock();
        stage = 'read';
        _baseline = _read();
        if (_baseline == null) return null;
        stage = 'format';
        final text = utf8.decode(_baseline!.bytes);
        final json = jsonDecode(text);
        _rejectDuplicateKeys(text);
        return WorkspacePreferences.fromJson(json);
      } on Object catch (error) {
        _blocked = true;
        // Keep any acquired lock until close, including corrupt-state sessions.
        throw WorkspacePreferencesException(
          'Cannot load arrangement ($stage); ${error is WorkspacePreferencesException ? error.message : 'Invalid data or unavailable native storage.'}',
        );
      }
    });
  }

  void _openDirectory() {
    final c = _native!;
    final root = _root ?? _home;
    if (root == null ||
        root.length > 4096 ||
        !root.startsWith('/') ||
        root.contains('\u0000')) {
      throw _error();
    }
    final parts = root.split('/').where((s) => s.isNotEmpty).toList();
    if (parts.isEmpty ||
        parts.length > 256 ||
        parts.any((s) => s == '.' || s == '..')) {
      throw _error();
    }
    var fd = c.open(-100, '/', _directory);
    try {
      c.safeDirectory(fd, ancestor: true);
      for (var i = 0; i < parts.length; i++) {
        final next = c.open(fd, parts[i], _directory);
        c.close(fd);
        fd = next;
        c.safeDirectory(fd, ancestor: i != parts.length - 1);
      }
      final children = [
        if (_root == null) ...['.local', 'state'],
        'smith-next',
        'client',
      ];
      for (final part in children) {
        final result = c.withName(part, (name) => c.mkdirat(fd, name, 0x1c0));
        if (result != 0 && c.errno != 17) throw _error();
        final next = c.open(fd, part, _directory);
        c.close(fd);
        fd = next;
        c.safeDirectory(fd, private: part == 'smith-next' || part == 'client');
      }
      _dir = fd;
      fd = -1;
    } finally {
      if (fd >= 0) c.close(fd);
    }
  }

  _Snapshot? _read() {
    final c = _native!;
    final fd = c.openChecked(_dir, _fileName, 0, 0, missingAllowed: true);
    if (fd < 0) return null;
    try {
      final stat = c.privateFile(fd);
      if (stat.size < 0 || stat.size > _limit) throw _error();
      final buffer = c.allocate(_limit + 1);
      try {
        var count = 0;
        while (count <= _limit) {
          final n = c.read(fd, buffer + count, _limit + 1 - count);
          if (n < 0 && c.errno == 4) continue;
          if (n < 0) throw _error();
          if (n == 0) break;
          count += n;
        }
        if (count > _limit) throw _error();
        return _Snapshot(
          stat.device,
          stat.inode,
          Uint8List.fromList(buffer.asTypedList(count)),
        );
      } finally {
        c.free(buffer.cast());
      }
    } finally {
      c.close(fd);
    }
  }

  @override
  Future<void> save(WorkspacePreferences value) {
    if (_closed || _blocked || _loading == null) return Future.error(_error());
    _pending = value;
    // One pending immutable snapshot, not an unbounded event/write queue.
    return _saving ??= Future<void>(() async {
      try {
        await _loading;
        while (!_closed && _pending != null) {
          final next = _pending!;
          _pending = null;
          _write(next);
        }
        if (_closed) throw _error();
      } on Object {
        _blocked = true;
        _pending = null;
        throw _error();
      } finally {
        _saving = null;
      }
    });
  }

  void _write(WorkspacePreferences value) {
    final c = _native!;
    c.safeDirectory(_dir, private: true);
    _verifyLock();
    final bytes = utf8.encode(jsonEncode(value.toJson()));
    if (bytes.length > _limit) throw _error();
    if (!_same(_read(), _baseline)) throw _error();
    final suffix = List.generate(
      16,
      (_) => Random.secure().nextInt(256).toRadixString(16).padLeft(2, '0'),
    ).join();
    final temporary = '.workspace-$suffix.tmp';
    var fd = -1;
    var ownedTemporary = false;
    try {
      fd = c.open(_dir, temporary, 1 | _create | _exclusive, 0x180);
      ownedTemporary = true;
      c.privateFile(fd);
      final buffer = c.allocate(bytes.length);
      try {
        buffer.asTypedList(bytes.length).setAll(0, bytes);
        var written = 0;
        while (written < bytes.length) {
          final n = c.write(fd, buffer + written, bytes.length - written);
          if (n < 0 && c.errno == 4) continue;
          if (n <= 0) throw _error();
          written += n;
        }
        if (c.fsync(fd) != 0) throw _error();
      } finally {
        c.free(buffer.cast());
      }
      if (!_same(_read(), _baseline)) throw _error();
      _verifyLock();
      final result = c.withName(
        temporary,
        (source) => c.withName(
          _fileName,
          (target) => c.renameat(_dir, source, _dir, target),
        ),
      );
      if (result != 0) throw _error();
      ownedTemporary = false;
      final stat = c.privateFile(fd);
      _baseline = _Snapshot(stat.device, stat.inode, bytes);
    } finally {
      if (fd >= 0) c.close(fd);
      if (ownedTemporary) {
        c.withName(temporary, (name) => c.unlinkat(_dir, name, 0));
      }
    }
  }

  void _verifyLock() {
    final c = _native!;
    final owned = c.privateFile(_lock);
    final named = c.open(_dir, '.workspace.lock', 0);
    try {
      final current = c.privateFile(named);
      if (owned.device != current.device || owned.inode != current.inode) {
        throw _error();
      }
    } finally {
      c.close(named);
    }
  }

  /// Fences queued saves immediately; does not publish after close is requested.
  /// Callers needing a saved final snapshot must await save before closing.
  @override
  Future<void> close() {
    _closed = true;
    _pending = null;
    return _closing ??= Future<void>(() async {
      try {
        await _loading;
      } on Object {
        /* Release even after failed load. */
      }
      try {
        await _saving;
      } on Object {
        /* Release even after failed save. */
      }
      final c = _native;
      if (c != null) {
        if (_lock >= 0) c.close(_lock);
        if (_dir >= 0) c.close(_dir);
      }
      _lock = _dir = -1;
    });
  }
}

WorkspacePreferencesException _error() => const WorkspacePreferencesException(
  'Storage unavailable or unsafe; this session cannot save its arrangement.',
);

class _Snapshot {
  _Snapshot(this.device, this.inode, this.bytes);
  final int device;
  final int inode;
  final List<int> bytes;
}

bool _same(_Snapshot? a, _Snapshot? b) {
  if (a == null || b == null) return a == b;
  if (a.device != b.device ||
      a.inode != b.inode ||
      a.bytes.length != b.bytes.length) {
    return false;
  }
  for (var i = 0; i < a.bytes.length; i++) {
    if (a.bytes[i] != b.bytes[i]) return false;
  }
  return true;
}

// jsonDecode accepts duplicate members; a strict schema must not silently do so.
// Decode first to validate syntax, then tokenize strings atomically (so braces
// and escaped quotes inside strings cannot affect object boundaries).
void _rejectDuplicateKeys(String text) {
  final stack = <Set<String>>[];
  final tokens = RegExp(r'"(?:[^"\\]|\\.)*"|[{}]');
  for (final match in tokens.allMatches(text)) {
    final token = match[0]!;
    if (token == '{') {
      stack.add({});
    } else if (token == '}') {
      stack.removeLast();
    } else if (text.substring(match.end).trimLeft().startsWith(':')) {
      if (!stack.last.add(jsonDecode(token) as String)) throw _error();
    }
  }
}

class _Stat {
  _Stat(this.device, this.inode, this.links, this.mode, this.uid, this.size);
  final int device, inode, links, mode, uid, size;
}

/// struct stat offsets below are the glibc Linux x86_64 ABI, gated before use.
class _LibC {
  final DynamicLibrary library = DynamicLibrary.open('libc.so.6');
  late final malloc = library
      .lookupFunction<
        Pointer<Void> Function(IntPtr),
        Pointer<Void> Function(int)
      >('malloc');
  late final free = library
      .lookupFunction<
        Void Function(Pointer<Void>),
        void Function(Pointer<Void>)
      >('free');
  late final openat = library
      .lookupFunction<
        Int32 Function(Int32, Pointer<Uint8>, Int32, Uint32),
        int Function(int, Pointer<Uint8>, int, int)
      >('openat');
  late final mkdirat = library
      .lookupFunction<
        Int32 Function(Int32, Pointer<Uint8>, Uint32),
        int Function(int, Pointer<Uint8>, int)
      >('mkdirat');
  late final renameat = library
      .lookupFunction<
        Int32 Function(Int32, Pointer<Uint8>, Int32, Pointer<Uint8>),
        int Function(int, Pointer<Uint8>, int, Pointer<Uint8>)
      >('renameat');
  late final unlinkat = library
      .lookupFunction<
        Int32 Function(Int32, Pointer<Uint8>, Int32),
        int Function(int, Pointer<Uint8>, int)
      >('unlinkat');
  late final close = library
      .lookupFunction<Int32 Function(Int32), int Function(int)>('close');
  late final fsync = library
      .lookupFunction<Int32 Function(Int32), int Function(int)>('fsync');
  late final flock = library
      .lookupFunction<Int32 Function(Int32, Int32), int Function(int, int)>(
        'flock',
      );
  late final fstat = library
      .lookupFunction<
        Int32 Function(Int32, Pointer<Uint8>),
        int Function(int, Pointer<Uint8>)
      >('fstat');
  late final getuid = library.lookupFunction<Uint32 Function(), int Function()>(
    'getuid',
  );
  late final read = library
      .lookupFunction<
        IntPtr Function(Int32, Pointer<Uint8>, IntPtr),
        int Function(int, Pointer<Uint8>, int)
      >('read');
  late final write = library
      .lookupFunction<
        IntPtr Function(Int32, Pointer<Uint8>, IntPtr),
        int Function(int, Pointer<Uint8>, int)
      >('write');
  late final errnoLocation = library
      .lookupFunction<Pointer<Int32> Function(), Pointer<Int32> Function()>(
        '__errno_location',
      );
  int get errno => errnoLocation().value;

  Pointer<Uint8> allocate(int length) {
    final pointer = malloc(length).cast<Uint8>();
    if (pointer == nullptr) throw _error();
    return pointer;
  }

  int withName(String name, int Function(Pointer<Uint8>) action) {
    final bytes = utf8.encode(name);
    final pointer = allocate(bytes.length + 1);
    try {
      pointer.asTypedList(bytes.length + 1)
        ..setAll(0, bytes)
        ..[bytes.length] = 0;
      return action(pointer);
    } finally {
      free(pointer.cast());
    }
  }

  int open(int parent, String name, int flags, [int mode = 0]) =>
      openChecked(parent, name, flags, mode);

  int openChecked(
    int parent,
    String name,
    int flags,
    int mode, {
    bool missingAllowed = false,
  }) {
    final fd = withName(
      name,
      (p) =>
          openat(parent, p, flags | _noFollow | _closeExec | _nonBlock, mode),
    );
    if (fd < 0 && !(missingAllowed && errno == 2)) {
      throw const WorkspacePreferencesException(
        'Cannot open a safe storage object.',
      );
    }
    return fd;
  }

  _Stat stat(int fd) {
    final buffer = allocate(144);
    try {
      if (fstat(fd, buffer) != 0) {
        throw const WorkspacePreferencesException(
          'Cannot inspect storage object.',
        );
      }
      final b = ByteData.sublistView(buffer.asTypedList(144));
      return _Stat(
        b.getUint64(0, Endian.host),
        b.getUint64(8, Endian.host),
        b.getUint64(16, Endian.host),
        b.getUint32(24, Endian.host),
        b.getUint32(28, Endian.host),
        b.getInt64(48, Endian.host),
      );
    } finally {
      free(buffer.cast());
    }
  }

  void safeDirectory(int fd, {bool ancestor = false, bool private = false}) {
    final s = stat(fd);
    final uid = getuid();
    final stickyRoot = ancestor && s.uid == 0 && s.mode & 0x200 != 0;
    if (s.mode & 0xf000 != 0x4000 ||
        (s.uid != uid && !(ancestor && s.uid == 0)) ||
        (s.mode & 0x12 != 0 && !stickyRoot) ||
        (private && s.mode & 0xfff != 0x1c0)) {
      throw const WorkspacePreferencesException(
        'Unsafe directory ownership or permissions.',
      );
    }
  }

  _Stat privateFile(int fd) {
    final s = stat(fd);
    if (s.mode & 0xf000 != 0x8000 ||
        s.uid != getuid() ||
        s.mode & 0xfff != 0x180 ||
        s.links != 1) {
      throw _error();
    }
    return s;
  }
}
