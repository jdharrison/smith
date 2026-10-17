import 'dart:convert';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Flutter generates version.json from pubspec.yaml during asset assembly.
/// The bundled pubspec supplies the declared version when generated metadata
/// is unavailable (for example during development or widget rendering).
@immutable
class BuildInfo {
  const BuildInfo._(this.version, this.buildNumber);
  const BuildInfo.development() : this._(null, null);

  final String? version;
  final String? buildNumber;

  String get buildVersion => version == null
      ? 'Version unavailable'
      : 'v$version${buildNumber == null ? '' : '+$buildNumber'}';

  static Future<BuildInfo> load({AssetBundle? bundle}) async {
    final assets = bundle ?? rootBundle;
    try {
      final info = BuildInfo.fromJson(
        jsonDecode(await assets.loadString('version.json')),
      );
      if (info.version != null) return info;
    } on FlutterError {
      // Generated metadata is not present in every Flutter execution mode.
    } on FormatException {
      // Fall back to the declared version, never an invented release number.
    }
    try {
      final pubspec = await assets.loadString('pubspec.yaml');
      final match = RegExp(
        r'^version:\s*(\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?)(?:\+(\d+))?\s*$',
        multiLine: true,
      ).firstMatch(pubspec);
      if (match != null) return BuildInfo._(match.group(1), match.group(2));
    } on FlutterError {
      // Keep missing metadata explicit if both sources are unavailable.
    }
    return const BuildInfo.development();
  }

  factory BuildInfo.fromJson(Object? value) {
    if (value is! Map<String, dynamic>) return const BuildInfo.development();
    final version = value['version'];
    final number = value['build_number'];
    if (version is! String ||
        !RegExp(r'^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$').hasMatch(version) ||
        (number != null &&
            (number is! String || !RegExp(r'^\d+$').hasMatch(number)))) {
      return const BuildInfo.development();
    }
    return BuildInfo._(version, number as String?);
  }
}
