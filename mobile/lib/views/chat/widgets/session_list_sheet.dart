import 'package:flutter/material.dart';
import '../../../api/ai_api.dart';
import '../../../l10n/strings.dart';
import '../../../theme/colors.dart';
import '../../../utils/secure_storage.dart';

/// Bottom sheet that shows a list of past chat sessions.
class SessionListSheet extends StatefulWidget {
  final void Function(String sessionId) onSessionTap;
  final VoidCallback onNewChat;

  const SessionListSheet({
    super.key,
    required this.onSessionTap,
    required this.onNewChat,
  });

  @override
  State<SessionListSheet> createState() => _SessionListSheetState();
}

class _SessionListSheetState extends State<SessionListSheet> {
  List<ChatSessionInfo>? _sessions;
  bool _loading = true;
  String? _error;

  @override
  void initState() {
    super.initState();
    _loadSessions();
  }

  Future<void> _loadSessions() async {
    final userId = await SecureStorage.getUserId();
    if (userId == null || userId.isEmpty) {
      setState(() {
        _loading = false;
        _sessions = [];
      });
      return;
    }

    final result = await AiApi.listSessions(userId: userId);
    if (!mounted) return;

    if (result.isSuccess && result.data != null) {
      setState(() {
        _sessions = result.data!;
        _loading = false;
      });
    } else {
      setState(() {
        _error = result.errorMessage;
        _loading = false;
      });
    }
  }

  Future<void> _deleteSession(ChatSessionInfo session) async {
    final userId = await SecureStorage.getUserId();
    if (userId == null || userId.isEmpty) return;

    if (!mounted) return;
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (ctx) => AlertDialog(
        backgroundColor: CwColors.bgCard,
        title: Text(S.deleteSession,
            style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w600, color: CwColors.ink1)),
        content: Text(S.deleteSessionConfirm,
            style: const TextStyle(fontSize: 14, color: CwColors.ink2)),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(false),
            child: Text(S.cancel, style: const TextStyle(color: CwColors.ink3)),
          ),
          TextButton(
            onPressed: () => Navigator.of(ctx).pop(true),
            child: Text(S.confirm, style: const TextStyle(color: CwColors.danger)),
          ),
        ],
      ),
    );

    if (confirmed != true) return;

    final result = await AiApi.deleteSession(sessionId: session.id, userId: userId);
    if (!mounted) return;

    if (result.isSuccess) {
      setState(() {
        _sessions?.removeWhere((s) => s.id == session.id);
      });
    }
  }

  @override
  Widget build(BuildContext context) {
    return DraggableScrollableSheet(
      initialChildSize: 0.6,
      minChildSize: 0.3,
      maxChildSize: 0.85,
      expand: false,
      builder: (context, scrollController) {
        return Container(
          decoration: const BoxDecoration(
            color: CwColors.bgPaper,
            borderRadius: BorderRadius.vertical(top: Radius.circular(16)),
          ),
          child: Column(
            children: [
              _buildHandle(),
              _buildHeader(),
              const Divider(height: 1, color: CwColors.line),
              Expanded(child: _buildBody(scrollController)),
            ],
          ),
        );
      },
    );
  }

  Widget _buildHandle() {
    return Padding(
      padding: const EdgeInsets.only(top: 8, bottom: 4),
      child: Center(
        child: Container(
          width: 36,
          height: 4,
          decoration: BoxDecoration(
            color: CwColors.ink4,
            borderRadius: BorderRadius.circular(2),
          ),
        ),
      ),
    );
  }

  Widget _buildHeader() {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
      child: Row(
        children: [
          const Icon(Icons.history_rounded, size: 20, color: CwColors.ink2),
          const SizedBox(width: 8),
          Text(
            S.chatHistory,
            style: const TextStyle(
              fontSize: 16,
              fontWeight: FontWeight.w600,
              color: CwColors.ink1,
            ),
          ),
          const Spacer(),
          GestureDetector(
            onTap: () {
              Navigator.of(context).pop();
              widget.onNewChat();
            },
            child: Container(
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
              decoration: BoxDecoration(
                color: CwColors.accentSoft,
                borderRadius: BorderRadius.circular(16),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  const Icon(Icons.add_rounded, size: 16, color: CwColors.accent),
                  const SizedBox(width: 4),
                  Text(
                    S.newChat,
                    style: const TextStyle(fontSize: 13, color: CwColors.accent, fontWeight: FontWeight.w500),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }

  Widget _buildBody(ScrollController scrollController) {
    if (_loading) {
      return const Center(
        child: CircularProgressIndicator(color: CwColors.accent, strokeWidth: 2),
      );
    }

    if (_error != null) {
      return Center(
        child: Text(_error!, style: const TextStyle(color: CwColors.ink3, fontSize: 14)),
      );
    }

    final sessions = _sessions ?? [];
    if (sessions.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.chat_bubble_outline_rounded, size: 40, color: CwColors.ink4),
            const SizedBox(height: 12),
            Text(S.noSessions, style: const TextStyle(color: CwColors.ink3, fontSize: 14)),
          ],
        ),
      );
    }

    return ListView.separated(
      controller: scrollController,
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      itemCount: sessions.length,
      separatorBuilder: (_, _) => const Divider(height: 1, color: CwColors.line),
      itemBuilder: (_, index) => _buildSessionTile(sessions[index]),
    );
  }

  Widget _buildSessionTile(ChatSessionInfo session) {
    final title = (session.title != null && session.title!.isNotEmpty)
        ? session.title!
        : 'Session ${session.id.substring(0, 8)}';
    final dateStr = _formatDate(session.updatedAt);

    return Dismissible(
      key: ValueKey(session.id),
      direction: DismissDirection.endToStart,
      background: Container(
        alignment: Alignment.centerRight,
        padding: const EdgeInsets.only(right: 20),
        color: CwColors.dangerSoft,
        child: const Icon(Icons.delete_outline_rounded, color: CwColors.danger),
      ),
      confirmDismiss: (_) async {
        await _deleteSession(session);
        return false; // We handle removal manually in _deleteSession
      },
      child: ListTile(
        contentPadding: const EdgeInsets.symmetric(horizontal: 4, vertical: 4),
        leading: Container(
          width: 36,
          height: 36,
          decoration: const BoxDecoration(
            color: CwColors.bgSubtle,
            shape: BoxShape.circle,
          ),
          child: const Icon(Icons.chat_rounded, size: 18, color: CwColors.ink3),
        ),
        title: Text(
          title,
          maxLines: 1,
          overflow: TextOverflow.ellipsis,
          style: const TextStyle(fontSize: 14, fontWeight: FontWeight.w500, color: CwColors.ink1),
        ),
        subtitle: Text(
          dateStr,
          style: const TextStyle(fontSize: 12, color: CwColors.ink4),
        ),
        trailing: GestureDetector(
          onTap: () => _deleteSession(session),
          child: const Icon(Icons.more_horiz_rounded, size: 18, color: CwColors.ink4),
        ),
        onTap: () {
          Navigator.of(context).pop();
          widget.onSessionTap(session.id);
        },
      ),
    );
  }

  String _formatDate(String isoDate) {
    if (isoDate.isEmpty) return '';
    try {
      final dt = DateTime.parse(isoDate);
      final now = DateTime.now();
      final diff = now.difference(dt);

      if (diff.inMinutes < 1) return S.justNow;
      if (diff.inHours < 1) return S.minutesAgo(diff.inMinutes);
      if (diff.inDays < 1) return S.hoursAgo(diff.inHours);
      if (diff.inDays < 30) return S.daysAgo(diff.inDays);
      return '${dt.year}-${dt.month.toString().padLeft(2, '0')}-${dt.day.toString().padLeft(2, '0')}';
    } catch (_) {
      return isoDate;
    }
  }
}
