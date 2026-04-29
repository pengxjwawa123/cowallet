import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'colors.dart';
import 'typography.dart';

ThemeData cwTheme() {
  return ThemeData(
    useMaterial3: true,
    brightness: Brightness.light,
    scaffoldBackgroundColor: CwColors.bgPaper,
    colorScheme: const ColorScheme.light(
      primary: CwColors.accent,
      onPrimary: Colors.white,
      secondary: CwColors.ink2,
      onSecondary: CwColors.bgPaper,
      surface: CwColors.bgCard,
      onSurface: CwColors.ink1,
      error: CwColors.danger,
      outline: CwColors.line,
      outlineVariant: CwColors.lineStrong,
    ),
    textTheme: CwTypography.textTheme,
    appBarTheme: const AppBarTheme(
      backgroundColor: CwColors.bgPaper,
      foregroundColor: CwColors.ink1,
      elevation: 0,
      scrolledUnderElevation: 0,
      systemOverlayStyle: SystemUiOverlayStyle.dark,
    ),
    cardTheme: CardThemeData(
      color: CwColors.bgCard,
      elevation: 0,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(16),
        side: const BorderSide(color: CwColors.line),
      ),
    ),
    bottomNavigationBarTheme: const BottomNavigationBarThemeData(
      backgroundColor: CwColors.bgPaper,
      selectedItemColor: CwColors.accent,
      unselectedItemColor: CwColors.ink4,
      type: BottomNavigationBarType.fixed,
      elevation: 0,
    ),
    filledButtonTheme: FilledButtonThemeData(
      style: FilledButton.styleFrom(
        backgroundColor: CwColors.accent,
        foregroundColor: Colors.white,
        minimumSize: const Size.fromHeight(52),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
        textStyle: const TextStyle(
          fontFamily: 'Inter',
          fontSize: 15,
          fontWeight: FontWeight.w600,
        ),
      ),
    ),
    outlinedButtonTheme: OutlinedButtonThemeData(
      style: OutlinedButton.styleFrom(
        foregroundColor: CwColors.ink1,
        minimumSize: const Size.fromHeight(52),
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(14)),
        side: const BorderSide(color: CwColors.lineStrong),
      ),
    ),
    dividerTheme: const DividerThemeData(
      color: CwColors.line,
      thickness: 1,
      space: 0,
    ),
  );
}
