import 'package:flutter/material.dart';
import '../theme/colors.dart';
import '../utils/secure_storage.dart';

/// Full-screen PIN verification dialog.
/// Returns true if PIN matches, false if user cancels.
class PinVerifyDialog extends StatefulWidget {
  final String reason;

  const PinVerifyDialog({super.key, required this.reason});

  /// Show the dialog and return whether authentication succeeded.
  static Future<bool> show(BuildContext context, {required String reason}) async {
    final result = await Navigator.of(context).push<bool>(
      PageRouteBuilder(
        opaque: true,
        pageBuilder: (context, a1, a2) => PinVerifyDialog(reason: reason),
        transitionsBuilder: (context, animation, secondaryAnimation, child) {
          return SlideTransition(
            position: Tween(begin: const Offset(0, 1), end: Offset.zero)
                .animate(CurvedAnimation(parent: animation, curve: Curves.easeOut)),
            child: child,
          );
        },
      ),
    );
    return result == true;
  }

  @override
  State<PinVerifyDialog> createState() => _PinVerifyDialogState();
}

class _PinVerifyDialogState extends State<PinVerifyDialog> {
  String _input = '';
  bool _error = false;
  int _attempts = 0;
  static const _maxAttempts = 5;

  Future<void> _onDigit(String digit) async {
    if (_input.length >= 6) return;
    setState(() {
      _input += digit;
      _error = false;
    });
    if (_input.length == 6) {
      await _verify();
    }
  }

  void _onBackspace() {
    if (_input.isEmpty) return;
    setState(() {
      _input = _input.substring(0, _input.length - 1);
      _error = false;
    });
  }

  Future<void> _verify() async {
    final stored = await SecureStorage.get('wallet_pin');
    if (_input == stored) {
      if (mounted) Navigator.of(context).pop(true);
    } else {
      _attempts++;
      if (_attempts >= _maxAttempts) {
        if (mounted) Navigator.of(context).pop(false);
        return;
      }
      setState(() {
        _error = true;
        _input = '';
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: CwColors.bgPaper,
      body: SafeArea(
        child: Column(
          children: [
            const SizedBox(height: 16),
            Align(
              alignment: Alignment.topRight,
              child: Padding(
                padding: const EdgeInsets.only(right: 16),
                child: IconButton(
                  icon: const Icon(Icons.close, color: CwColors.ink3),
                  onPressed: () => Navigator.of(context).pop(false),
                ),
              ),
            ),
            const Spacer(flex: 2),
            Icon(Icons.lock_outline, size: 48, color: CwColors.accent),
            const SizedBox(height: 24),
            Text(
              widget.reason,
              style: const TextStyle(fontSize: 16, color: CwColors.ink2),
            ),
            const SizedBox(height: 32),
            Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: List.generate(6, (i) {
                final filled = i < _input.length;
                return Container(
                  width: 16,
                  height: 16,
                  margin: const EdgeInsets.symmetric(horizontal: 8),
                  decoration: BoxDecoration(
                    shape: BoxShape.circle,
                    color: filled
                        ? (_error ? CwColors.danger : CwColors.accent)
                        : Colors.transparent,
                    border: Border.all(
                      color: _error
                          ? CwColors.danger
                          : (filled ? CwColors.accent : CwColors.ink4),
                      width: 2,
                    ),
                  ),
                );
              }),
            ),
            if (_error)
              Padding(
                padding: const EdgeInsets.only(top: 16),
                child: Text(
                  'PIN码错误，还剩${_maxAttempts - _attempts}次机会',
                  style: const TextStyle(color: CwColors.danger, fontSize: 13),
                ),
              ),
            const Spacer(flex: 1),
            _buildNumPad(),
            const SizedBox(height: 40),
          ],
        ),
      ),
    );
  }

  Widget _buildNumPad() {
    return Column(
      children: [
        for (final row in [
          ['1', '2', '3'],
          ['4', '5', '6'],
          ['7', '8', '9'],
          ['', '0', '⌫']
        ])
          Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              children: row.map((key) {
                if (key.isEmpty) return const SizedBox(width: 72, height: 56);
                return GestureDetector(
                  onTap: () {
                    if (key == '⌫') {
                      _onBackspace();
                    } else {
                      _onDigit(key);
                    }
                  },
                  child: Container(
                    width: 72,
                    height: 56,
                    margin: const EdgeInsets.symmetric(horizontal: 8),
                    decoration: BoxDecoration(
                      color: CwColors.bgPaper,
                      borderRadius: BorderRadius.circular(12),
                    ),
                    child: Center(
                      child: key == '⌫'
                          ? const Icon(Icons.backspace_outlined,
                              size: 22, color: CwColors.ink2)
                          : Text(
                              key,
                              style: const TextStyle(
                                fontSize: 24,
                                fontWeight: FontWeight.w500,
                                color: CwColors.ink1,
                              ),
                            ),
                    ),
                  ),
                );
              }).toList(),
            ),
          ),
      ],
    );
  }
}
