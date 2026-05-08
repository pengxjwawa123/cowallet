import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/cw_orb.dart';
import '../../main.dart';
import '../../services/locator.dart';
import '../../api/ai_api.dart';
import '../../models/ai_response.dart';
import 'widgets/balance_widget.dart';
import 'widgets/receive_widget.dart';
import 'widgets/send_confirm_widget.dart';
import 'widgets/tx_result_widget.dart';

// ---------------------------------------------------------------------------
// Message model
// ---------------------------------------------------------------------------

enum ChatMsgKind { user, ai, thinking, widget }

enum WidgetType { balance, receive, sendConfirm, txResult }

class ChatMsg {
  final ChatMsgKind kind;
  final String text;
  final WidgetType? widgetType;
  final Map<String, dynamic> widgetData;
  final String? toolCallId;
  bool confirmed;
  bool loading;

  ChatMsg({
    required this.kind,
    this.text = '',
    this.widgetType,
    this.widgetData = const {},
    this.toolCallId,
    this.confirmed = false,
    this.loading = false,
  });
}

// ---------------------------------------------------------------------------
// Suggestion pills
// ---------------------------------------------------------------------------

class _Suggestion {
  final String zhText;
  final String enText;
  const _Suggestion({required this.zhText, required this.enText});
  String text(Lang lang) => lang == Lang.zh ? zhText : enText;
}

const _suggestions = [
  _Suggestion(zhText: '我的余额是多少', enText: "What's my balance"),
  _Suggestion(zhText: '我的收款地址', enText: 'Show my address'),
  _Suggestion(zhText: '最近的交易记录', enText: 'Recent transactions'),
];

// ---------------------------------------------------------------------------
// ChatView
// ---------------------------------------------------------------------------

class ChatView extends StatefulWidget {
  const ChatView({super.key});

  @override
  State<ChatView> createState() => _ChatViewState();
}

class _ChatViewState extends State<ChatView> {
  final _controller = TextEditingController();
  final _scrollController = ScrollController();
  final _messages = <ChatMsg>[];
  final _history = <Map<String, dynamic>>[];
  final _focusNode = FocusNode();

  @override
  void dispose() {
    _controller.dispose();
    _scrollController.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  Lang get _lang => CowalletApp.of(context).lang;
  bool get _isEmpty => _messages.isEmpty;

  void _scrollToBottom() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scrollController.hasClients) {
        _scrollController.animateTo(
          _scrollController.position.maxScrollExtent,
          duration: const Duration(milliseconds: 250),
          curve: Curves.easeOut,
        );
      }
    });
  }

  // -- send logic ------------------------------------------------------------

  void _send([String? override]) async {
    final text = (override ?? _controller.text).trim();
    if (text.isEmpty) return;

    setState(() {
      _messages.add(ChatMsg(kind: ChatMsgKind.user, text: text));
      _controller.clear();
      _messages.add(ChatMsg(kind: ChatMsgKind.thinking));
    });
    _scrollToBottom();

    _history.add({"role": "user", "content": text});

    final address = CowalletApp.of(context).walletAddress;
    final result = await AiApi.chat(
      message: text,
      history: _history,
      walletAddress: address.isNotEmpty ? address : null,
    );

    if (!mounted) return;

    setState(() {
      _messages.removeWhere((m) => m.kind == ChatMsgKind.thinking);
    });

    if (result.isSuccess && result.data != null) {
      final response = result.data!;
      _handleAiResponse(response);
      _history.add({"role": "assistant", "content": response.message});
    } else {
      setState(() {
        _messages.add(ChatMsg(
          kind: ChatMsgKind.ai,
          text: result.errorMessage ?? '请求失败，请稍后重试',
        ));
      });
    }
    _scrollToBottom();
  }

  void _handleAiResponse(AiChatResponse response) {
    // Add AI text message
    if (response.message.isNotEmpty) {
      _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: response.message));
    }

    // Render inline widgets for READ tool results
    for (final toolResult in response.toolResults) {
      if (response.needsConfirmation.contains(toolResult.toolId)) continue;
      if (!toolResult.success) continue;

      switch (toolResult.toolName) {
        case 'get_balance':
          _messages.add(ChatMsg(
            kind: ChatMsgKind.widget,
            widgetType: WidgetType.balance,
            widgetData: toolResult.result,
          ));
          break;
        case 'get_wallet_address':
          final addr = toolResult.result['address'] as String? ??
              CowalletApp.of(context).walletAddress;
          if (addr.isNotEmpty) {
            _messages.add(ChatMsg(
              kind: ChatMsgKind.widget,
              widgetType: WidgetType.receive,
              widgetData: {'address': addr},
            ));
          }
          break;
      }
    }

    // Render WRITE tool confirmations (send_transaction)
    for (final toolCall in response.toolCalls) {
      if (!response.needsConfirmation.contains(toolCall.id)) continue;

      if (toolCall.name == 'send_transaction') {
        final params = toolCall.parameters;
        final gasResult = response.getResultForTool('estimate_gas');
        _messages.add(ChatMsg(
          kind: ChatMsgKind.widget,
          widgetType: WidgetType.sendConfirm,
          widgetData: {
            'to_address': params['to_address'] ?? '',
            'amount': params['value'] ?? '0',
            'token': params['token_address'] != null ? 'Token' : 'ETH',
            'token_address': params['token_address'],
            'gas_estimate': gasResult?.result['gas_estimate']?.toString(),
          },
          toolCallId: toolCall.id,
        ));
      }
    }

    setState(() {});
  }

  Future<void> _onSendConfirm(int index) async {
    final msg = _messages[index];
    setState(() => msg.loading = true);

    final params = {
      'to': msg.widgetData['to_address'] as String? ?? '',
      'amount': msg.widgetData['amount'] as String? ?? '0',
      'token': msg.widgetData['token'] as String? ?? 'ETH',
    };

    final result = await Services.intent.execute('transfer', params);

    if (!mounted) return;
    setState(() {
      msg.loading = false;
      msg.confirmed = true;

      if (result.success) {
        _messages.add(ChatMsg(
          kind: ChatMsgKind.widget,
          widgetType: WidgetType.txResult,
          widgetData: {
            'tx_hash': result.data['txHash'] ?? '',
            'success': true,
            'amount': params['amount'],
            'token': params['token'],
          },
        ));
        _messages.add(ChatMsg(
          kind: ChatMsgKind.ai,
          text: result.message,
        ));
      } else {
        _messages.add(ChatMsg(
          kind: ChatMsgKind.ai,
          text: '⚠ ${result.message}',
        ));
      }
    });
    _scrollToBottom();
  }

  void _onSendDeny(int index) {
    setState(() {
      _messages[index].confirmed = true;
      _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: '好的，已取消转账。'));
    });
    _scrollToBottom();
  }

  // -- build -----------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(child: _isEmpty ? _buildEmptyState() : _buildConversation()),
        _buildComposer(),
      ],
    );
  }

  Widget _buildEmptyState() {
    return Center(
      child: SingleChildScrollView(
        padding: const EdgeInsets.symmetric(horizontal: 32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const CwOrb(size: 70, breathing: true),
            const SizedBox(height: 24),
            Text(
              S.chatEmpty,
              style: const TextStyle(
                fontFamily: 'NotoSerifSC',
                fontSize: 22,
                fontWeight: FontWeight.w600,
                color: CwColors.ink1,
                height: 1.3,
              ),
            ),
            const SizedBox(height: 8),
            Text(
              S.chatEmptySub,
              style: const TextStyle(
                fontSize: 14,
                color: CwColors.ink3,
                height: 1.5,
              ),
            ),
            const SizedBox(height: 28),
            Wrap(
              spacing: 8,
              runSpacing: 8,
              alignment: WrapAlignment.center,
              children: _suggestions.map((s) => _suggestionPill(s)).toList(),
            ),
          ],
        ),
      ),
    );
  }

  Widget _suggestionPill(_Suggestion s) {
    return GestureDetector(
      onTap: () => _send(s.text(_lang)),
      child: Container(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
        decoration: BoxDecoration(
          color: CwColors.bgCard,
          border: Border.all(color: CwColors.line),
          borderRadius: BorderRadius.circular(999),
        ),
        child: Text(
          s.text(_lang),
          style: const TextStyle(fontSize: 13, color: CwColors.ink2, height: 1.3),
        ),
      ),
    );
  }

  Widget _buildConversation() {
    return ListView.builder(
      controller: _scrollController,
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 8),
      itemCount: _messages.length,
      itemBuilder: (_, i) {
        final msg = _messages[i];
        switch (msg.kind) {
          case ChatMsgKind.user:
            return _buildUserBubble(msg);
          case ChatMsgKind.ai:
            return _buildAiMessage(msg);
          case ChatMsgKind.thinking:
            return _buildThinkingDots();
          case ChatMsgKind.widget:
            return _buildInlineWidget(msg, i);
        }
      },
    );
  }

  Widget _buildUserBubble(ChatMsg msg) {
    return Align(
      alignment: Alignment.centerRight,
      child: Container(
        margin: const EdgeInsets.only(bottom: 12),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        constraints: BoxConstraints(
          maxWidth: MediaQuery.of(context).size.width * 0.75,
        ),
        decoration: const BoxDecoration(
          color: Color(0xFF141008),
          borderRadius: BorderRadius.only(
            topLeft: Radius.circular(18),
            topRight: Radius.circular(18),
            bottomLeft: Radius.circular(18),
            bottomRight: Radius.circular(4),
          ),
        ),
        child: Text(
          msg.text,
          style: const TextStyle(fontSize: 14, height: 1.5, color: Colors.white),
        ),
      ),
    );
  }

  Widget _buildAiMessage(ChatMsg msg) {
    return Align(
      alignment: Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.only(bottom: 12),
        constraints: BoxConstraints(
          maxWidth: MediaQuery.of(context).size.width * 0.8,
        ),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 6, height: 6,
                  decoration: const BoxDecoration(
                    color: CwColors.accent,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 5),
                const Text(
                  'COWALLET',
                  style: TextStyle(
                    fontFamily: 'JetBrainsMono',
                    fontSize: 10,
                    letterSpacing: 1.0,
                    fontWeight: FontWeight.w600,
                    color: CwColors.ink3,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 6),
            Text(
              msg.text,
              style: const TextStyle(fontSize: 14, height: 1.55, color: CwColors.ink1),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildThinkingDots() {
    return Align(
      alignment: Alignment.centerLeft,
      child: Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: _ThinkingDots(),
      ),
    );
  }

  Widget _buildInlineWidget(ChatMsg msg, int index) {
    return Align(
      alignment: Alignment.centerLeft,
      child: Container(
        constraints: BoxConstraints(
          maxWidth: MediaQuery.of(context).size.width * 0.85,
        ),
        child: _widgetForType(msg, index),
      ),
    );
  }

  Widget _widgetForType(ChatMsg msg, int index) {
    switch (msg.widgetType) {
      case WidgetType.balance:
        return ChatBalanceWidget(data: msg.widgetData);
      case WidgetType.receive:
        return ChatReceiveWidget(address: msg.widgetData['address'] ?? '');
      case WidgetType.sendConfirm:
        return ChatSendConfirmWidget(
          toAddress: msg.widgetData['to_address'] ?? '',
          amount: msg.widgetData['amount'] ?? '0',
          token: msg.widgetData['token'] ?? 'ETH',
          gasEstimate: msg.widgetData['gas_estimate'],
          loading: msg.loading,
          resolved: msg.confirmed,
          onConfirm: () => _onSendConfirm(index),
          onDeny: () => _onSendDeny(index),
        );
      case WidgetType.txResult:
        return ChatTxResultWidget(
          txHash: msg.widgetData['tx_hash'] ?? '',
          success: msg.widgetData['success'] ?? true,
          amount: msg.widgetData['amount'],
          token: msg.widgetData['token'],
        );
      default:
        return const SizedBox.shrink();
    }
  }

  Widget _buildComposer() {
    return Container(
      padding: const EdgeInsets.fromLTRB(8, 8, 8, 8),
      decoration: const BoxDecoration(
        color: CwColors.bgPaper,
        border: Border(top: BorderSide(color: CwColors.line)),
      ),
      child: SafeArea(
        top: false,
        child: Row(
          children: [
            Expanded(
              child: TextField(
                controller: _controller,
                focusNode: _focusNode,
                decoration: InputDecoration(
                  hintText: S.composerHint,
                  hintStyle: const TextStyle(color: CwColors.ink4, fontSize: 14),
                  border: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(24),
                    borderSide: const BorderSide(color: CwColors.line),
                  ),
                  enabledBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(24),
                    borderSide: const BorderSide(color: CwColors.line),
                  ),
                  focusedBorder: OutlineInputBorder(
                    borderRadius: BorderRadius.circular(24),
                    borderSide: const BorderSide(color: CwColors.accent),
                  ),
                  contentPadding: const EdgeInsets.symmetric(
                    horizontal: 16, vertical: 10,
                  ),
                  isDense: true,
                ),
                textInputAction: TextInputAction.send,
                onSubmitted: (_) => _send(),
              ),
            ),
            const SizedBox(width: 4),
            SizedBox(
              width: 40,
              height: 40,
              child: IconButton(
                padding: EdgeInsets.zero,
                icon: const Icon(Icons.send_rounded, size: 20),
                color: CwColors.accent,
                onPressed: _send,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

// ---------------------------------------------------------------------------
// Thinking dots animation
// ---------------------------------------------------------------------------

class _ThinkingDots extends StatefulWidget {
  @override
  State<_ThinkingDots> createState() => _ThinkingDotsState();
}

class _ThinkingDotsState extends State<_ThinkingDots>
    with SingleTickerProviderStateMixin {
  late AnimationController _ctrl;

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1200),
    )..repeat();
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
      builder: (_, __) {
        return Row(
          mainAxisSize: MainAxisSize.min,
          children: List.generate(3, (i) {
            final delay = i * 0.2;
            final t = (_ctrl.value - delay).clamp(0.0, 1.0);
            final progress = (t * 3.14159).clamp(0.0, 3.14159);
            final sinVal = _sin(progress);
            final opacity = 0.3 + 0.7 * sinVal;
            final offset = -3.0 * sinVal;

            return Padding(
              padding: EdgeInsets.only(right: i < 2 ? 4 : 0),
              child: Transform.translate(
                offset: Offset(0, offset),
                child: Opacity(
                  opacity: opacity,
                  child: Container(
                    width: 6, height: 6,
                    decoration: const BoxDecoration(
                      color: CwColors.ink4,
                      shape: BoxShape.circle,
                    ),
                  ),
                ),
              ),
            );
          }),
        );
      },
    );
  }

  double _sin(double x) {
    final n = x % 3.14159;
    return 4 * n * (3.14159 - n) / (3.14159 * 3.14159);
  }
}
