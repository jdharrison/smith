#!/bin/sh
set -eu
dart format --output=none --set-exit-if-changed lib test
flutter analyze --no-pub
flutter test --no-pub
