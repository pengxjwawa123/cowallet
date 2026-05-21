import 'package:flutter/material.dart';
import 'package:fluttertoast/fluttertoast.dart';
import '../theme/colors.dart';

void showTopToast(BuildContext context, String message, {Color? backgroundColor}) {
  Fluttertoast.showToast(
    msg: message,
    toastLength: Toast.LENGTH_SHORT,
    gravity: ToastGravity.TOP,
    backgroundColor: backgroundColor ?? CwColors.ink1,
    textColor: CwColors.bgPaper,
    fontSize: 14,
  );
}
