import 'package:flutter/material.dart';
import 'package:flutter_svg/flutter_svg.dart';

import 'smith_theme.dart';

/// Local outline glyph only; interactive callers provide a labelled 48px target.
class SmithIcon extends StatelessWidget {
  const SmithIcon(
    this.name, {
    super.key,
    this.size = 24,
    this.color,
    this.semanticLabel,
  }) : assert(size > 0);

  static const names = <String>{
    'chevron-right',
    'message-square',
    'settings',
    'archive',
    'plus',
    'ellipsis-vertical',
    'pencil',
    'trash-2',
    'send',
    'x',
    'menu',
    'rotate-ccw',
    'info',
    'check',
    'chevron-down',
    'layers',
    'git-branch',
  };

  final String name;
  final double size;
  final Color? color;
  final String? semanticLabel;

  @override
  Widget build(BuildContext context) {
    if (!names.contains(name)) {
      throw ArgumentError.value(name, 'name', 'Unknown bundled Smith icon');
    }
    final tint =
        color ?? IconTheme.of(context).color ?? SmithColors.textSecondary;
    return SvgPicture.asset(
      'assets/icons/$name.svg',
      width: size,
      height: size,
      colorFilter: ColorFilter.mode(tint, BlendMode.srcIn),
      semanticsLabel: semanticLabel,
      excludeFromSemantics: semanticLabel == null,
    );
  }
}
