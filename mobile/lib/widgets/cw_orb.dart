import 'dart:math';
import 'package:flutter/material.dart';
import '../theme/colors.dart';

class CwOrb extends StatefulWidget {
  final double size;
  final bool breathing;
  final bool thinking;

  const CwOrb({
    super.key,
    this.size = 120,
    this.breathing = true,
    this.thinking = false,
  });

  @override
  State<CwOrb> createState() => _CwOrbState();
}

class _CwOrbState extends State<CwOrb> with SingleTickerProviderStateMixin {
  late AnimationController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
      vsync: this,
      duration: Duration(milliseconds: widget.thinking ? 1200 : 3000),
    )..repeat(reverse: !widget.thinking);
  }

  @override
  void dispose() {
    _ctrl.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _ctrl,
      builder: (_, _) {
        final scale = widget.breathing
            ? 1.0 + sin(_ctrl.value * 2 * pi) * 0.03
            : 1.0;
        final glow = widget.thinking ? 0.15 + _ctrl.value * 0.1 : 0.15;
        return Transform.scale(
          scale: scale,
          child: SizedBox(
            width: widget.size,
            height: widget.size,
            child: CustomPaint(
              painter: _OrbPainter(glowOpacity: glow),
            ),
          ),
        );
      },
    );
  }
}

class _OrbPainter extends CustomPainter {
  final double glowOpacity;

  _OrbPainter({required this.glowOpacity});

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = size.width / 2;

    // Outer glow
    final glowPaint = Paint()
      ..shader = RadialGradient(
        center: Alignment.center,
        radius: 1.0,
        colors: [
          CwColors.accent.withValues(alpha: glowOpacity),
          CwColors.accent.withValues(alpha: 0),
        ],
      ).createShader(Rect.fromCircle(center: center, radius: radius));
    canvas.drawCircle(center, radius, glowPaint);

    // Main orb body
    final bodyPaint = Paint()
      ..shader = RadialGradient(
        center: const Alignment(-0.24, -0.4),
        radius: 1.36,
        colors: const [
          Color(0xFFFFD4BC),
          Color(0xFFD97757),
          Color(0xFF8A3F2A),
        ],
        stops: const [0.0, 0.45, 1.0],
      ).createShader(Rect.fromCircle(center: center, radius: radius * 0.82));
    canvas.drawCircle(center, radius * 0.82, bodyPaint);

    // Highlight ellipse
    final hlCenter = Offset(size.width * 0.39, size.height * 0.33);
    final hlPaint = Paint()
      ..shader = RadialGradient(
        center: Alignment.center,
        radius: 1.0,
        colors: [
          const Color(0xFFFFF8EF).withValues(alpha: 0.5),
          const Color(0xFFFFF8EF).withValues(alpha: 0),
        ],
      ).createShader(Rect.fromCenter(
        center: hlCenter,
        width: radius * 0.56,
        height: radius * 0.36,
      ));
    canvas.drawOval(
      Rect.fromCenter(center: hlCenter, width: radius * 0.56, height: radius * 0.36),
      hlPaint,
    );
  }

  @override
  bool shouldRepaint(_OrbPainter old) => old.glowOpacity != glowOpacity;
}
