import datetime
import json
import os
from pathlib import Path
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[3]
SDK = Path.home() / 'snap/flutter/common/flutter'
DART = SDK / 'bin/cache/dart-sdk/bin/dart'
TOOLS = SDK / 'bin/cache/flutter_tools.snapshot'
EVIDENCE = ROOT / 'docs/evidence/dev-navigation/preferences'
OWNED = ['lib/workspace_preferences.dart', 'test/workspace_preferences_test.dart', 'test/workspace_restore_test.dart']
name = sys.argv[1]
commands = {
    'format': [str(DART), 'format', *OWNED],
    'format-check': [str(DART), 'format', '--output=none', '--set-exit-if-changed', 'lib/main.dart', *OWNED],
    'analyze': [str(DART), 'analyze', 'lib/workspace_preferences.dart', 'lib/main.dart', 'test/workspace_preferences_test.dart', 'test/workspace_restore_test.dart'],
    'preferences': [str(DART), str(TOOLS), 'test', '--no-pub', '--reporter', 'expanded', 'test/workspace_preferences_test.dart'],
    'restore': [str(DART), str(TOOLS), 'test', '--no-pub', '--reporter', 'expanded', 'test/workspace_restore_test.dart'],
    'session-only': [str(DART), str(TOOLS), 'test', '--no-pub', '--reporter', 'expanded', 'test/new_window_test.dart'],
}
command = commands[name]
env = dict(os.environ)
env['FLUTTER_SUPPRESS_ANALYTICS'] = 'true'
env['DART_SUPPRESS_ANALYTICS'] = 'true'
def sanitized(text):
    return text.replace(str(ROOT), '<repo>').replace(str(SDK), '<cached-flutter-sdk>').replace(str(Path.home()), '<home>')
started = datetime.datetime.now(datetime.timezone.utc).isoformat()
try:
    result = subprocess.run(command, cwd=ROOT / 'client', env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, timeout=180)
    output, code = result.stdout, result.returncode
except subprocess.TimeoutExpired as error:
    output = (error.stdout or b'').decode(errors='replace') if isinstance(error.stdout, bytes) else error.stdout or ''
    output += '\nTIMEOUT after 180 seconds\n'
    code = 124
record = {'command': sanitized(' '.join(command)), 'cwd': 'client', 'started_utc': started, 'exit': code, 'timeout_seconds': 180}
log = json.dumps(record, indent=2) + '\n' + sanitized(output)
(EVIDENCE / (name + '.log')).write_text(log)
print(log)
sys.exit(code)
