import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../l10n/strings.dart';
import '../../services/locator.dart';
import '../../theme/colors.dart';
import '../../widgets/top_toast.dart';

/// Backup shard export/import view.
///
/// Export flow: Enter password -> confirm password -> encrypt via Rust FFI -> show QR / save file.
/// Import flow: Paste data / scan QR -> enter password -> decrypt via Rust FFI -> success/fail.
class BackupShardView extends StatefulWidget {
  const BackupShardView({super.key});

  @override
  State<BackupShardView> createState() => _BackupShardViewState();
}

class _BackupShardViewState extends State<BackupShardView>
    with SingleTickerProviderStateMixin {
  late TabController _tabController;

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 2, vsync: this);
  }

  @override
  void dispose() {
    _tabController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: CwColors.bgPaper,
      appBar: AppBar(
        title: Text(
          S.backupExport,
          style: const TextStyle(
            fontFamily: 'NotoSerifSC',
            fontSize: 16,
            fontWeight: FontWeight.w600,
          ),
        ),
        backgroundColor: CwColors.bgPaper,
        elevation: 0,
        bottom: TabBar(
          controller: _tabController,
          labelColor: CwColors.accent,
          unselectedLabelColor: CwColors.ink3,
          indicatorColor: CwColors.accent,
          tabs: [
            Tab(text: S.backupExport),
            Tab(text: S.backupImport),
          ],
        ),
      ),
      body: TabBarView(
        controller: _tabController,
        children: const [
          _ExportTab(),
          _ImportTab(),
        ],
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Export Tab
// ---------------------------------------------------------------------------

class _ExportTab extends StatefulWidget {
  const _ExportTab();

  @override
  State<_ExportTab> createState() => _ExportTabState();
}

class _ExportTabState extends State<_ExportTab> {
  final _passwordController = TextEditingController();
  final _confirmController = TextEditingController();
  bool _isExporting = false;
  String? _exportedData;
  String? _error;
  bool _obscurePassword = true;
  bool _obscureConfirm = true;

  @override
  void dispose() {
    _passwordController.dispose();
    _confirmController.dispose();
    super.dispose();
  }

  Future<void> _doExport() async {
    final password = _passwordController.text;
    final confirm = _confirmController.text;

    if (password.length < 8) {
      setState(() => _error = S.backupPasswordTooShort);
      return;
    }
    if (password != confirm) {
      setState(() => _error = S.backupPasswordMismatch);
      return;
    }

    setState(() {
      _isExporting = true;
      _error = null;
      _exportedData = null;
    });

    try {
      final encrypted = await Services.backup.exportEncrypted(password);
      if (mounted) {
        setState(() {
          _exportedData = encrypted;
          _isExporting = false;
        });
        showTopToast(context, S.backupExportSuccess, backgroundColor: CwColors.success);
      }
    } catch (e) {
      if (mounted) {
        setState(() {
          _error = '${S.backupExportFailed}: $e';
          _isExporting = false;
        });
      }
    }
  }

  Future<void> _copyToClipboard() async {
    if (_exportedData == null) return;
    await Clipboard.setData(ClipboardData(text: _exportedData!));
    if (mounted) {
      showTopToast(context, S.backupCopied, backgroundColor: CwColors.success);
    }
  }

  Future<void> _saveToFile() async {
    if (_exportedData == null) return;

    try {
      final filePath = await Services.backup.exportEncryptedToFile(
        _passwordController.text,
      );
      if (mounted) {
        showTopToast(
          context,
          S.backupFileSaved(filePath),
          backgroundColor: CwColors.success,
        );
      }
    } catch (e) {
      if (mounted) {
        showTopToast(
          context,
          '${S.backupExportFailed}: $e',
          backgroundColor: CwColors.danger,
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        // Description
        Text(
          S.backupExportSub,
          style: const TextStyle(fontSize: 13, color: CwColors.ink3),
        ),
        const SizedBox(height: 20),

        // Password field
        TextField(
          controller: _passwordController,
          obscureText: _obscurePassword,
          decoration: InputDecoration(
            labelText: S.backupPasswordHint,
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
            suffixIcon: IconButton(
              icon: Icon(
                _obscurePassword ? Icons.visibility_off : Icons.visibility,
                size: 20,
              ),
              onPressed: () => setState(() => _obscurePassword = !_obscurePassword),
            ),
          ),
        ),
        const SizedBox(height: 12),

        // Confirm password field
        TextField(
          controller: _confirmController,
          obscureText: _obscureConfirm,
          decoration: InputDecoration(
            labelText: S.backupPasswordConfirmHint,
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
            suffixIcon: IconButton(
              icon: Icon(
                _obscureConfirm ? Icons.visibility_off : Icons.visibility,
                size: 20,
              ),
              onPressed: () => setState(() => _obscureConfirm = !_obscureConfirm),
            ),
          ),
        ),
        const SizedBox(height: 8),

        // Error message
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Text(
              _error!,
              style: const TextStyle(fontSize: 12, color: CwColors.danger),
            ),
          ),

        // Export button
        const SizedBox(height: 12),
        SizedBox(
          width: double.infinity,
          height: 48,
          child: ElevatedButton(
            onPressed: _isExporting ? null : _doExport,
            style: ElevatedButton.styleFrom(
              backgroundColor: CwColors.accent,
              foregroundColor: Colors.white,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12),
              ),
            ),
            child: _isExporting
                ? const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: Colors.white,
                    ),
                  )
                : Text(S.backupExport),
          ),
        ),

        // Exported data display
        if (_exportedData != null) ...[
          const SizedBox(height: 24),
          Container(
            padding: const EdgeInsets.all(14),
            decoration: BoxDecoration(
              color: CwColors.bgCard,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: CwColors.line),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  S.backupEncryptedData,
                  style: const TextStyle(
                    fontSize: 12,
                    fontWeight: FontWeight.w600,
                    color: CwColors.ink2,
                  ),
                ),
                const SizedBox(height: 8),
                SelectableText(
                  _exportedData!,
                  style: const TextStyle(
                    fontFamily: 'JetBrainsMono',
                    fontSize: 11,
                    color: CwColors.ink2,
                  ),
                  maxLines: 6,
                ),
                const SizedBox(height: 12),
                Row(
                  children: [
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _copyToClipboard,
                        icon: const Icon(Icons.copy, size: 16),
                        label: Text(S.backupCopyToClipboard, style: const TextStyle(fontSize: 12)),
                        style: OutlinedButton.styleFrom(
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(8),
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _saveToFile,
                        icon: const Icon(Icons.save_alt, size: 16),
                        label: Text(S.backupSaveToFile, style: const TextStyle(fontSize: 12)),
                        style: OutlinedButton.styleFrom(
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(8),
                          ),
                        ),
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
        ],
      ],
    );
  }
}

// ---------------------------------------------------------------------------
// Import Tab
// ---------------------------------------------------------------------------

class _ImportTab extends StatefulWidget {
  const _ImportTab();

  @override
  State<_ImportTab> createState() => _ImportTabState();
}

class _ImportTabState extends State<_ImportTab> {
  final _dataController = TextEditingController();
  final _passwordController = TextEditingController();
  bool _isImporting = false;
  String? _error;
  bool _obscurePassword = true;

  @override
  void dispose() {
    _dataController.dispose();
    _passwordController.dispose();
    super.dispose();
  }

  Future<void> _pasteFromClipboard() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    if (data?.text != null && data!.text!.isNotEmpty) {
      _dataController.text = data.text!;
      setState(() {});
    }
  }

  Future<void> _doImport() async {
    final encryptedData = _dataController.text.trim();
    final password = _passwordController.text;

    if (encryptedData.isEmpty) {
      setState(() => _error = S.backupPasteData);
      return;
    }
    if (password.isEmpty) {
      setState(() => _error = S.backupPasswordTooShort);
      return;
    }

    setState(() {
      _isImporting = true;
      _error = null;
    });

    try {
      await Services.backup.importEncrypted(encryptedData, password);
      if (mounted) {
        setState(() => _isImporting = false);
        showTopToast(context, S.backupImportSuccess, backgroundColor: CwColors.success);
        Navigator.pop(context, true);
      }
    } catch (e) {
      if (mounted) {
        final errorMsg = e.toString();
        setState(() {
          _error = errorMsg.contains('wrong password') || errorMsg.contains('decryption failed')
              ? S.backupWrongPassword
              : '${S.backupImportFailed}: $e';
          _isImporting = false;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(20),
      children: [
        // Description
        Text(
          S.backupImportSub,
          style: const TextStyle(fontSize: 13, color: CwColors.ink3),
        ),
        const SizedBox(height: 20),

        // Encrypted data input
        TextField(
          controller: _dataController,
          maxLines: 5,
          decoration: InputDecoration(
            labelText: S.backupEncryptedData,
            alignLabelWithHint: true,
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
            suffixIcon: IconButton(
              icon: const Icon(Icons.paste, size: 20),
              onPressed: _pasteFromClipboard,
              tooltip: S.backupPasteData,
            ),
          ),
          style: const TextStyle(
            fontFamily: 'JetBrainsMono',
            fontSize: 11,
          ),
        ),
        const SizedBox(height: 12),

        // Password field
        TextField(
          controller: _passwordController,
          obscureText: _obscurePassword,
          decoration: InputDecoration(
            labelText: S.backupPasswordHint,
            border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
            suffixIcon: IconButton(
              icon: Icon(
                _obscurePassword ? Icons.visibility_off : Icons.visibility,
                size: 20,
              ),
              onPressed: () => setState(() => _obscurePassword = !_obscurePassword),
            ),
          ),
        ),
        const SizedBox(height: 8),

        // Error message
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(bottom: 8),
            child: Text(
              _error!,
              style: const TextStyle(fontSize: 12, color: CwColors.danger),
            ),
          ),

        // Import button
        const SizedBox(height: 12),
        SizedBox(
          width: double.infinity,
          height: 48,
          child: ElevatedButton(
            onPressed: _isImporting ? null : _doImport,
            style: ElevatedButton.styleFrom(
              backgroundColor: CwColors.accent,
              foregroundColor: Colors.white,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(12),
              ),
            ),
            child: _isImporting
                ? const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: Colors.white,
                    ),
                  )
                : Text(S.backupImport),
          ),
        ),
      ],
    );
  }
}
