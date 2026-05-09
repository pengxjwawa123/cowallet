import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../main.dart';
import '../../widgets/top_toast.dart';

class ReceiveView extends StatelessWidget {
  const ReceiveView({super.key});

  @override
  Widget build(BuildContext context) {
    final address = CowalletApp.of(context).walletAddress;
    final hasAddress = address.isNotEmpty;
    final displayAddress = hasAddress
        ? '${address.substring(0, 6)}...${address.substring(address.length - 4)}'
        : '';

    return Scaffold(
      appBar: AppBar(
        title:
            Text(S.receiveTitle, style: Theme.of(context).textTheme.titleLarge),
      ),
      body: Padding(
        padding: const EdgeInsets.all(20),
        child: Column(
          children: [
            const Spacer(),
            Container(
              padding: const EdgeInsets.all(24),
              decoration: BoxDecoration(
                color: CwColors.bgCard,
                borderRadius: BorderRadius.circular(20),
                border: Border.all(color: CwColors.line),
              ),
              child: Column(
                children: [
                  if (hasAddress)
                    QrImageView(
                      data: address,
                      version: QrVersions.auto,
                      size: 200,
                      backgroundColor: Colors.white,
                      eyeStyle: const QrEyeStyle(
                        eyeShape: QrEyeShape.square,
                        color: CwColors.ink1,
                      ),
                      dataModuleStyle: const QrDataModuleStyle(
                        dataModuleShape: QrDataModuleShape.square,
                        color: CwColors.ink1,
                      ),
                    )
                  else
                    Container(
                      width: 200,
                      height: 200,
                      decoration: BoxDecoration(
                        color: CwColors.bgSubtle,
                        borderRadius: BorderRadius.circular(12),
                      ),
                      child: Center(
                        child: Text(
                          S.createWalletFirst,
                          style: const TextStyle(color: CwColors.ink4),
                        ),
                      ),
                    ),
                  const SizedBox(height: 20),
                  Text(
                    hasAddress ? displayAddress : S.createWalletFirst,
                    style: Theme.of(context).textTheme.labelLarge?.copyWith(
                          fontSize: 16,
                          color: hasAddress ? null : CwColors.ink4,
                        ),
                  ),
                  if (hasAddress) ...[
                    const SizedBox(height: 4),
                    Text(
                      address,
                      style: Theme.of(context).textTheme.bodySmall?.copyWith(
                            fontFamily: 'JetBrainsMono',
                            fontSize: 10,
                            color: CwColors.ink3,
                          ),
                      textAlign: TextAlign.center,
                    ),
                  ],
                  const SizedBox(height: 8),
                  Text(
                    'Base · Ethereum L2',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ],
              ),
            ),
            const SizedBox(height: 24),
            Row(
              children: [
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: hasAddress
                        ? () {
                            Clipboard.setData(ClipboardData(text: address));
                            showTopToast(context, S.addressCopied);
                          }
                        : null,
                    icon: const Icon(Icons.copy, size: 18),
                    label: Text(S.copyAddress),
                  ),
                ),
                const SizedBox(width: 12),
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: hasAddress ? () {} : null,
                    icon: const Icon(Icons.share, size: 18),
                    label: Text(S.share),
                  ),
                ),
              ],
            ),
            const Spacer(flex: 2),
          ],
        ),
      ),
    );
  }
}
