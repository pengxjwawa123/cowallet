import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';
import '../../../theme/colors.dart';

class ChatReceiveWidget extends StatelessWidget {
  final String address;

  const ChatReceiveWidget({super.key, required this.address});

  @override
  Widget build(BuildContext context) {
    final shortAddr = address.length >= 10
        ? '${address.substring(0, 6)}...${address.substring(address.length - 4)}'
        : address;

    return Container(
      margin: const EdgeInsets.only(bottom: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: CwColors.bgCard,
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: CwColors.line),
      ),
      child: Column(
        children: [
          Container(
            padding: const EdgeInsets.all(12),
            decoration: BoxDecoration(
              color: Colors.white,
              borderRadius: BorderRadius.circular(12),
            ),
            child: QrImageView(
              data: address,
              version: QrVersions.auto,
              size: 160,
              eyeStyle: const QrEyeStyle(
                eyeShape: QrEyeShape.square,
                color: CwColors.ink1,
              ),
              dataModuleStyle: const QrDataModuleStyle(
                dataModuleShape: QrDataModuleShape.square,
                color: CwColors.ink1,
              ),
            ),
          ),
          const SizedBox(height: 12),
          Text(
            shortAddr,
            style: const TextStyle(
              fontSize: 14,
              fontWeight: FontWeight.w500,
              color: CwColors.ink2,
            ),
          ),
          const SizedBox(height: 4),
          Text(
            'Base · Ethereum L2',
            style: TextStyle(fontSize: 11, color: CwColors.ink4),
          ),
          const SizedBox(height: 12),
          SizedBox(
            width: double.infinity,
            child: OutlinedButton.icon(
              onPressed: () {
                Clipboard.setData(ClipboardData(text: address));
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('地址已复制')),
                );
              },
              icon: const Icon(Icons.copy, size: 16),
              label: const Text('复制地址'),
              style: OutlinedButton.styleFrom(
                foregroundColor: CwColors.ink2,
                side: const BorderSide(color: CwColors.line),
                shape: RoundedRectangleBorder(
                  borderRadius: BorderRadius.circular(10),
                ),
              ),
            ),
          ),
        ],
      ),
    );
  }
}
