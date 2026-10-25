import 'package:flutter/material.dart';

abstract final class SmithColors {
  static const background = Color(0xFF031514);
  static const surface = Color(0xFF04201D);
  static const surfaceRaised = Color(0xFF092A27);
  static const surfaceSelected = Color(0xFF063D34);
  static const stroke = Color(0xFF0B6557);
  static const strokeSubtle = Color(0xFF164A43);
  static const accent = Color(0xFF00E9C6);
  static const accentBright = Color(0xFF00FFE0);
  static const textPrimary = Color(0xFFE3F0EE);
  static const textSecondary = Color(0xFF99B5B2);
  // Slightly lifted from the comp to meet 4.5:1 on raised surfaces.
  static const textMuted = Color(0xFF789290);
  static const statusOnline = Color(0xFF00E6CA);
  static const danger = Color(0xFFFF4D58);
  static const userMarker = Color(0xFFE4C7EB);
}

/// Shared geometry, including accessible hit areas around compact glyphs.
@immutable
class SmithTokens extends ThemeExtension<SmithTokens> {
  const SmithTokens({
    this.gap = 8,
    this.gutter = 16,
    this.radius = 6,
    this.hitTarget = 48,
    this.readingWidth = 960,
    this.bubbleWidth = 720,
  });

  final double gap;
  final double gutter;
  final double radius;
  final double hitTarget;
  final double readingWidth;
  final double bubbleWidth;

  @override
  SmithTokens copyWith({
    double? gap,
    double? gutter,
    double? radius,
    double? hitTarget,
    double? readingWidth,
    double? bubbleWidth,
  }) => SmithTokens(
    gap: gap ?? this.gap,
    gutter: gutter ?? this.gutter,
    radius: radius ?? this.radius,
    hitTarget: hitTarget ?? this.hitTarget,
    readingWidth: readingWidth ?? this.readingWidth,
    bubbleWidth: bubbleWidth ?? this.bubbleWidth,
  );

  @override
  SmithTokens lerp(covariant SmithTokens? other, double t) {
    if (other == null) return this;
    double mix(double a, double b) => a + (b - a) * t;
    return SmithTokens(
      gap: mix(gap, other.gap),
      gutter: mix(gutter, other.gutter),
      radius: mix(radius, other.radius),
      hitTarget: mix(hitTarget, other.hitTarget),
      readingWidth: mix(readingWidth, other.readingWidth),
      bubbleWidth: mix(bubbleWidth, other.bubbleWidth),
    );
  }
}

abstract final class SmithTheme {
  static const defaultFontFamily = 'JetBrains Mono';
  static const alternativeFontFamily = 'IBM Plex Mono';

  static ThemeData dark({String fontFamily = defaultFontFamily}) {
    const scheme = ColorScheme.dark(
      primary: SmithColors.accent,
      onPrimary: SmithColors.background,
      secondary: SmithColors.accentBright,
      onSecondary: SmithColors.background,
      surface: SmithColors.surface,
      onSurface: SmithColors.textPrimary,
      onSurfaceVariant: SmithColors.textSecondary,
      surfaceContainerLowest: SmithColors.background,
      surfaceContainerLow: SmithColors.surface,
      surfaceContainer: SmithColors.surfaceRaised,
      surfaceContainerHigh: SmithColors.surfaceRaised,
      surfaceContainerHighest: SmithColors.surfaceSelected,
      outline: SmithColors.stroke,
      outlineVariant: SmithColors.strokeSubtle,
      error: SmithColors.danger,
      onError: SmithColors.background,
      surfaceTint: Colors.transparent,
    );
    final base = ThemeData(
      useMaterial3: true,
      brightness: Brightness.dark,
      colorScheme: scheme,
      fontFamily: fontFamily,
      scaffoldBackgroundColor: SmithColors.background,
      materialTapTargetSize: MaterialTapTargetSize.padded,
      visualDensity: VisualDensity.standard,
    );
    final border = OutlineInputBorder(
      borderRadius: BorderRadius.circular(6),
      borderSide: const BorderSide(color: SmithColors.stroke),
    );
    final buttonStyle = ButtonStyle(
      minimumSize: const WidgetStatePropertyAll(Size(48, 48)),
      foregroundColor: WidgetStateProperty.resolveWith(
        (states) => states.contains(WidgetState.disabled)
            ? SmithColors.textMuted
            : SmithColors.accent,
      ),
      overlayColor: WidgetStateProperty.resolveWith((states) {
        if (states.contains(WidgetState.disabled)) return Colors.transparent;
        if (states.contains(WidgetState.pressed)) {
          return SmithColors.accent.withValues(alpha: 0.18);
        }
        if (states.contains(WidgetState.focused) ||
            states.contains(WidgetState.hovered)) {
          return SmithColors.accent.withValues(alpha: 0.10);
        }
        return Colors.transparent;
      }),
      side: WidgetStateProperty.resolveWith(
        (states) => BorderSide(
          color:
              states.contains(WidgetState.focused) &&
                  !states.contains(WidgetState.disabled)
              ? SmithColors.accentBright
              : Colors.transparent,
        ),
      ),
      shape: WidgetStatePropertyAll(
        RoundedRectangleBorder(borderRadius: BorderRadius.circular(6)),
      ),
    );
    return base.copyWith(
      extensions: const [SmithTokens()],
      textTheme: base.textTheme.copyWith(
        titleLarge: base.textTheme.titleLarge?.copyWith(
          fontSize: 22,
          fontWeight: FontWeight.w600,
        ),
        titleMedium: base.textTheme.titleMedium?.copyWith(
          fontSize: 16,
          fontWeight: FontWeight.w500,
        ),
        bodyLarge: base.textTheme.bodyLarge?.copyWith(
          fontSize: 14,
          height: 1.5,
        ),
        bodyMedium: base.textTheme.bodyMedium?.copyWith(
          fontSize: 14,
          height: 1.5,
        ),
        bodySmall: base.textTheme.bodySmall?.copyWith(
          fontSize: 12,
          height: 1.5,
          color: SmithColors.textSecondary,
        ),
        labelLarge: base.textTheme.labelLarge?.copyWith(
          fontSize: 14,
          fontWeight: FontWeight.w500,
        ),
        labelSmall: base.textTheme.labelSmall?.copyWith(
          fontSize: 12,
          color: SmithColors.textSecondary,
        ),
      ),
      dividerColor: SmithColors.strokeSubtle,
      disabledColor: SmithColors.textMuted,
      focusColor: SmithColors.surfaceSelected,
      hoverColor: SmithColors.surfaceRaised,
      iconTheme: const IconThemeData(
        color: SmithColors.textSecondary,
        size: 24,
      ),
      iconButtonTheme: IconButtonThemeData(style: buttonStyle),
      textButtonTheme: TextButtonThemeData(style: buttonStyle),
      outlinedButtonTheme: OutlinedButtonThemeData(style: buttonStyle),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: SmithColors.surface,
        border: border,
        enabledBorder: border,
        focusedBorder: border.copyWith(
          borderSide: const BorderSide(
            color: SmithColors.accentBright,
            width: 2,
          ),
        ),
        errorBorder: border.copyWith(
          borderSide: const BorderSide(color: SmithColors.danger),
        ),
        hintStyle: const TextStyle(color: SmithColors.textMuted),
        contentPadding: const EdgeInsets.all(16),
      ),
      textSelectionTheme: const TextSelectionThemeData(
        cursorColor: SmithColors.accent,
        selectionColor: SmithColors.surfaceSelected,
        selectionHandleColor: SmithColors.accent,
      ),
      appBarTheme: const AppBarTheme(
        backgroundColor: SmithColors.background,
        foregroundColor: SmithColors.textPrimary,
        elevation: 0,
        scrolledUnderElevation: 0,
      ),
      dialogTheme: const DialogThemeData(
        backgroundColor: SmithColors.surfaceRaised,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
      ),
      popupMenuTheme: const PopupMenuThemeData(
        color: SmithColors.surfaceRaised,
        surfaceTintColor: Colors.transparent,
        elevation: 0,
      ),
      tooltipTheme: const TooltipThemeData(
        decoration: BoxDecoration(
          color: SmithColors.surfaceRaised,
          borderRadius: BorderRadius.all(Radius.circular(4)),
        ),
        textStyle: TextStyle(color: SmithColors.textPrimary, fontSize: 12),
      ),
    );
  }
}
