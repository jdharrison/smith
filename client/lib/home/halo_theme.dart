import 'package:flutter/material.dart';

abstract final class HaloColors {
  static const background = Color(0xFF080F0C);
  static const surface = Color(0xFF101A14);
  static const border = Color(0xFF26382C);
  static const primary = Color(0xFFEDF3EE);
  static const secondary = Color(0xFF91A395);
  static const accent = Color(0xFFA4D995);
  static const danger = Color(0xFFFF9B94);
  static const warning = Color(0xFFF0C674);
  static const info = Color(0xFF91C9E8);
}

ThemeData buildHaloTheme() {
  final base = ThemeData(
    useMaterial3: true,
    brightness: Brightness.dark,
    scaffoldBackgroundColor: HaloColors.background,
    colorScheme: const ColorScheme.dark(
      primary: HaloColors.accent,
      onPrimary: HaloColors.background,
      secondary: HaloColors.accent,
      onSecondary: HaloColors.background,
      surface: HaloColors.surface,
      onSurface: HaloColors.primary,
      error: HaloColors.danger,
      outline: HaloColors.border,
    ),
  );

  final textTheme = base.textTheme.apply(
    bodyColor: HaloColors.primary,
    displayColor: HaloColors.primary,
  );

  return base.copyWith(
    textTheme: textTheme,
    dividerColor: HaloColors.border,
    focusColor: HaloColors.accent.withValues(alpha: 0.16),
    hoverColor: HaloColors.accent.withValues(alpha: 0.08),
    splashColor: HaloColors.accent.withValues(alpha: 0.10),
    inputDecorationTheme: InputDecorationTheme(
      filled: true,
      fillColor: HaloColors.surface,
      hintStyle: textTheme.bodyMedium?.copyWith(color: HaloColors.secondary),
      enabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: const BorderSide(color: HaloColors.border),
      ),
      focusedBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: const BorderSide(color: HaloColors.accent, width: 1.5),
      ),
      disabledBorder: OutlineInputBorder(
        borderRadius: BorderRadius.circular(12),
        borderSide: const BorderSide(color: HaloColors.border),
      ),
    ),
    cardTheme: CardThemeData(
      color: HaloColors.surface,
      elevation: 0,
      margin: EdgeInsets.zero,
      shape: RoundedRectangleBorder(
        side: const BorderSide(color: HaloColors.border),
        borderRadius: BorderRadius.circular(12),
      ),
    ),
    iconButtonTheme: IconButtonThemeData(
      style: IconButton.styleFrom(
        foregroundColor: HaloColors.primary,
        minimumSize: const Size(48, 48),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: HaloColors.primary,
        minimumSize: const Size(48, 48),
        side: const BorderSide(color: HaloColors.border),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        foregroundColor: HaloColors.background,
        backgroundColor: HaloColors.accent,
        minimumSize: const Size(48, 48),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    ),
    textButtonTheme: TextButtonThemeData(
      style: TextButton.styleFrom(
        foregroundColor: HaloColors.accent,
        minimumSize: const Size(48, 48),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      ),
    ),
    navigationBarTheme: NavigationBarThemeData(
      backgroundColor: HaloColors.surface,
      indicatorColor: HaloColors.accent.withValues(alpha: 0.14),
      labelTextStyle: WidgetStatePropertyAll(
        textTheme.labelMedium?.copyWith(color: HaloColors.primary),
      ),
    ),
    progressIndicatorTheme: const ProgressIndicatorThemeData(
      color: HaloColors.accent,
      linearTrackColor: HaloColors.border,
      circularTrackColor: HaloColors.border,
    ),
  );
}
