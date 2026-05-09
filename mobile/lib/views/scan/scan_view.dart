import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../router/app_router.dart';

class ScanView extends StatefulWidget {
  const ScanView({super.key});

  @override
  State<ScanView> createState() => _ScanViewState();
}

class _ScanViewState extends State<ScanView> {
  final MobileScannerController _controller = MobileScannerController(
    detectionSpeed: DetectionSpeed.normal,
    facing: CameraFacing.back,
  );

  bool _scanned = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  void _onDetect(BarcodeCapture capture) {
    if (_scanned) return;
    final barcode = capture.barcodes.firstOrNull;
    if (barcode == null || barcode.rawValue == null) return;

    setState(() => _scanned = true);
    final raw = barcode.rawValue!;
    final message = _parseQrData(raw);
    _navigateToChatWithMessage(message);
  }

  String _parseQrData(String raw) {
    // EIP-681 URI: ethereum:0x...?value=...&...
    if (raw.startsWith('ethereum:')) {
      return _parseEip681(raw);
    }

    // Plain Ethereum address
    if (_isEthAddress(raw)) {
      return S.scanTransferTo(raw);
    }

    // Generic text/URL — send as-is
    return raw;
  }

  String _parseEip681(String uri) {
    // Format: ethereum:<address>[@chainId][/<function>]?[key=value&...]
    final withoutScheme = uri.substring('ethereum:'.length);

    // Extract address (up to @, /, or ?)
    String address = withoutScheme;
    String params = '';

    final questionIdx = withoutScheme.indexOf('?');
    if (questionIdx != -1) {
      address = withoutScheme.substring(0, questionIdx);
      params = withoutScheme.substring(questionIdx + 1);
    }

    // Strip chainId (@xxx) and function (/xxx)
    final atIdx = address.indexOf('@');
    if (atIdx != -1) {
      address = address.substring(0, atIdx);
    }
    final slashIdx = address.indexOf('/');
    if (slashIdx != -1) {
      address = address.substring(0, slashIdx);
    }

    if (!_isEthAddress(address)) {
      return uri; // fallback: send raw
    }

    // Parse query params
    final paramMap = Uri.splitQueryString(params);

    final value = paramMap['value'];
    final token = paramMap['token'] ?? 'ETH';

    if (value != null && value.isNotEmpty) {
      // Convert wei to ETH for display if token is ETH
      String displayAmount = value;
      if (token == 'ETH') {
        try {
          final wei = BigInt.parse(value);
          final ethValue = wei / BigInt.from(10).pow(18);
          final remainder = wei % BigInt.from(10).pow(18);
          if (remainder == BigInt.zero) {
            displayAmount = ethValue.toString();
          } else {
            // Show as decimal
            final full = wei.toString().padLeft(19, '0');
            final intPart = full.substring(0, full.length - 18);
            final fracPart = full.substring(full.length - 18).replaceAll(RegExp(r'0+$'), '');
            displayAmount = '$intPart.$fracPart';
          }
        } catch (_) {
          displayAmount = value;
        }
      }
      return S.scanTransferAmount(displayAmount, token, address);
    }

    return S.scanTransferTo(address);
  }

  bool _isEthAddress(String s) {
    return RegExp(r'^0x[0-9a-fA-F]{40}$').hasMatch(s);
  }

  void _navigateToChatWithMessage(String message) {
    Navigator.of(context).pop();
    // After popping back to AppShell, send the message to chat
    WidgetsBinding.instance.addPostFrameCallback((_) {
      final ctx = context;
      if (ctx.mounted) {
        AppShell.goToChatAndSend(ctx, message);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: Colors.black,
      body: Stack(
        children: [
          // Camera preview
          MobileScanner(
            controller: _controller,
            onDetect: _onDetect,
            errorBuilder: (context, error, child) {
              return _buildPermissionDenied(context);
            },
          ),

          // Scan overlay
          _buildOverlay(context),

          // Top bar with back & flash buttons
          _buildTopBar(context),

          // Bottom hint
          _buildBottomHint(context),
        ],
      ),
    );
  }

  Widget _buildPermissionDenied(BuildContext context) {
    return Container(
      color: Colors.black,
      child: Center(
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Icon(Icons.camera_alt_outlined, color: CwColors.ink4, size: 48),
              const SizedBox(height: 16),
              Text(
                S.scanPermissionDenied,
                style: const TextStyle(color: Colors.white, fontSize: 16),
                textAlign: TextAlign.center,
              ),
              const SizedBox(height: 24),
              TextButton(
                onPressed: () => Navigator.of(context).pop(),
                child: Text(
                  S.back,
                  style: const TextStyle(color: CwColors.accent, fontSize: 16),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _buildOverlay(BuildContext context) {
    return CustomPaint(
      painter: _ScanOverlayPainter(),
      child: const SizedBox.expand(),
    );
  }

  Widget _buildTopBar(BuildContext context) {
    return SafeArea(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 8),
        child: Row(
          children: [
            // Back button
            IconButton(
              onPressed: () => Navigator.of(context).pop(),
              icon: const Icon(Icons.arrow_back_ios_new, color: Colors.white, size: 22),
            ),
            const Spacer(),
            Text(
              S.scanTitle,
              style: const TextStyle(
                color: Colors.white,
                fontSize: 17,
                fontWeight: FontWeight.w600,
              ),
            ),
            const Spacer(),
            // Flashlight toggle
            ValueListenableBuilder<MobileScannerState>(
              valueListenable: _controller,
              builder: (context, state, child) {
                final isOn = state.torchState == TorchState.on;
                return IconButton(
                  onPressed: () => _controller.toggleTorch(),
                  icon: Icon(
                    isOn ? Icons.flash_on : Icons.flash_off,
                    color: isOn ? CwColors.gold : Colors.white,
                    size: 22,
                  ),
                  tooltip: isOn ? S.scanFlashOn : S.scanFlashOff,
                );
              },
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildBottomHint(BuildContext context) {
    return Positioned(
      bottom: 100,
      left: 0,
      right: 0,
      child: Center(
        child: Text(
          S.scanHint,
          style: const TextStyle(
            color: Colors.white70,
            fontSize: 14,
          ),
        ),
      ),
    );
  }
}

/// Custom painter for the scan overlay with a transparent center square
class _ScanOverlayPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final scanAreaSize = size.width * 0.65;
    final left = (size.width - scanAreaSize) / 2;
    final top = (size.height - scanAreaSize) / 2 - 40;

    // Semi-transparent background
    final bgPaint = Paint()..color = Colors.black.withValues(alpha: 0.5);

    // Draw the four darkened regions around the scan area
    // Top
    canvas.drawRect(Rect.fromLTRB(0, 0, size.width, top), bgPaint);
    // Bottom
    canvas.drawRect(Rect.fromLTRB(0, top + scanAreaSize, size.width, size.height), bgPaint);
    // Left
    canvas.drawRect(Rect.fromLTRB(0, top, left, top + scanAreaSize), bgPaint);
    // Right
    canvas.drawRect(Rect.fromLTRB(left + scanAreaSize, top, size.width, top + scanAreaSize), bgPaint);

    // Draw corner lines
    final cornerPaint = Paint()
      ..color = CwColors.accent
      ..strokeWidth = 3
      ..style = PaintingStyle.stroke
      ..strokeCap = StrokeCap.round;

    const cornerLen = 24.0;
    const radius = 8.0;

    // Top-left corner
    canvas.drawPath(
      Path()
        ..moveTo(left, top + cornerLen)
        ..lineTo(left, top + radius)
        ..quadraticBezierTo(left, top, left + radius, top)
        ..lineTo(left + cornerLen, top),
      cornerPaint,
    );

    // Top-right corner
    canvas.drawPath(
      Path()
        ..moveTo(left + scanAreaSize - cornerLen, top)
        ..lineTo(left + scanAreaSize - radius, top)
        ..quadraticBezierTo(left + scanAreaSize, top, left + scanAreaSize, top + radius)
        ..lineTo(left + scanAreaSize, top + cornerLen),
      cornerPaint,
    );

    // Bottom-left corner
    canvas.drawPath(
      Path()
        ..moveTo(left, top + scanAreaSize - cornerLen)
        ..lineTo(left, top + scanAreaSize - radius)
        ..quadraticBezierTo(left, top + scanAreaSize, left + radius, top + scanAreaSize)
        ..lineTo(left + cornerLen, top + scanAreaSize),
      cornerPaint,
    );

    // Bottom-right corner
    canvas.drawPath(
      Path()
        ..moveTo(left + scanAreaSize - cornerLen, top + scanAreaSize)
        ..lineTo(left + scanAreaSize - radius, top + scanAreaSize)
        ..quadraticBezierTo(left + scanAreaSize, top + scanAreaSize, left + scanAreaSize, top + scanAreaSize - radius)
        ..lineTo(left + scanAreaSize, top + scanAreaSize - cornerLen),
      cornerPaint,
    );
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}
