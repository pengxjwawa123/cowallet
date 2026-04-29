import 'package:flutter/material.dart';
import 'colors.dart';

abstract final class CwTypography {
  static const _serif = 'NotoSerifSC';
  static const _serifEn = 'Fraunces';
  static const _sans = 'Inter';
  static const _mono = 'JetBrainsMono';

  static final textTheme = TextTheme(
    displayLarge: TextStyle(
      fontFamily: _serif,
      fontSize: 32,
      fontWeight: FontWeight.w600,
      color: CwColors.ink1,
      height: 1.3,
    ),
    displayMedium: TextStyle(
      fontFamily: _serif,
      fontSize: 26,
      fontWeight: FontWeight.w600,
      color: CwColors.ink1,
      height: 1.3,
    ),
    headlineLarge: TextStyle(
      fontFamily: _serif,
      fontSize: 22,
      fontWeight: FontWeight.w600,
      color: CwColors.ink1,
      height: 1.4,
    ),
    headlineMedium: TextStyle(
      fontFamily: _serifEn,
      fontSize: 20,
      fontWeight: FontWeight.w500,
      color: CwColors.ink1,
      height: 1.4,
    ),
    titleLarge: TextStyle(
      fontFamily: _sans,
      fontSize: 17,
      fontWeight: FontWeight.w600,
      color: CwColors.ink1,
    ),
    titleMedium: TextStyle(
      fontFamily: _sans,
      fontSize: 15,
      fontWeight: FontWeight.w500,
      color: CwColors.ink2,
    ),
    bodyLarge: TextStyle(
      fontFamily: _sans,
      fontSize: 15,
      fontWeight: FontWeight.w400,
      color: CwColors.ink1,
      height: 1.6,
    ),
    bodyMedium: TextStyle(
      fontFamily: _sans,
      fontSize: 13,
      fontWeight: FontWeight.w400,
      color: CwColors.ink2,
      height: 1.5,
    ),
    bodySmall: TextStyle(
      fontFamily: _sans,
      fontSize: 12,
      fontWeight: FontWeight.w400,
      color: CwColors.ink3,
    ),
    labelLarge: TextStyle(
      fontFamily: _mono,
      fontSize: 13,
      fontWeight: FontWeight.w500,
      color: CwColors.ink2,
      letterSpacing: 0.5,
    ),
    labelMedium: TextStyle(
      fontFamily: _mono,
      fontSize: 11,
      fontWeight: FontWeight.w400,
      color: CwColors.ink3,
    ),
  );
}
