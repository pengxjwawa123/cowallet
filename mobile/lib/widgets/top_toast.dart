import 'package:flutter/material.dart';
import '../theme/colors.dart';

void showTopToast(BuildContext context, String message, {Color? backgroundColor}) {
  final bg = backgroundColor ?? CwColors.ink1;
  final overlay = Overlay.of(context);
  final topPadding = MediaQuery.of(context).padding.top;

  late OverlayEntry entry;
  entry = OverlayEntry(
    builder: (context) => _TopToastWidget(
      message: message,
      backgroundColor: bg,
      topPadding: topPadding,
      onDismiss: () => entry.remove(),
    ),
  );

  overlay.insert(entry);
}

class _TopToastWidget extends StatefulWidget {
  final String message;
  final Color backgroundColor;
  final double topPadding;
  final VoidCallback onDismiss;

  const _TopToastWidget({
    required this.message,
    required this.backgroundColor,
    required this.topPadding,
    required this.onDismiss,
  });

  @override
  State<_TopToastWidget> createState() => _TopToastWidgetState();
}

class _TopToastWidgetState extends State<_TopToastWidget>
    with SingleTickerProviderStateMixin {
  late AnimationController _controller;
  late Animation<double> _opacity;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      duration: const Duration(milliseconds: 200),
      vsync: this,
    );
    _opacity = CurvedAnimation(parent: _controller, curve: Curves.easeIn);
    _controller.forward();

    Future.delayed(const Duration(seconds: 2), () {
      if (mounted) {
        _controller.reverse().then((_) {
          if (mounted) widget.onDismiss();
        });
      }
    });
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Positioned(
      top: widget.topPadding + 10,
      left: 20,
      right: 20,
      child: FadeTransition(
        opacity: _opacity,
        child: Material(
          color: Colors.transparent,
          child: Container(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
            decoration: BoxDecoration(
              color: widget.backgroundColor,
              borderRadius: BorderRadius.circular(12),
            ),
            child: Text(
              widget.message,
              style: const TextStyle(
                color: CwColors.bgPaper,
                fontSize: 14,
                fontWeight: FontWeight.w500,
              ),
            ),
          ),
        ),
      ),
    );
  }
}
