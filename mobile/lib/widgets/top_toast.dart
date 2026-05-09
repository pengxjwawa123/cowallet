import 'package:flutter/material.dart';
import '../theme/colors.dart';

void showTopToast(BuildContext context, String message, {Color? backgroundColor}) {
  final bg = backgroundColor ?? CwColors.ink1;
  ScaffoldMessenger.of(context)
    ..clearSnackBars()
    ..showSnackBar(
      SnackBar(
        content: Text(
          message,
          style: const TextStyle(
            color: CwColors.bgPaper,
            fontSize: 14,
            fontWeight: FontWeight.w500,
          ),
        ),
        backgroundColor: bg,
        behavior: SnackBarBehavior.floating,
        elevation: 0,
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        margin: EdgeInsets.only(
          bottom: MediaQuery.of(context).size.height -
              MediaQuery.of(context).padding.top - 100,
          left: 20,
          right: 20,
        ),
        duration: const Duration(seconds: 2),
      ),
    );
}
