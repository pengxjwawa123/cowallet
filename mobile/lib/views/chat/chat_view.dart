import 'dart:async';
import 'package:flutter/material.dart';
import 'package:flutter_markdown/flutter_markdown.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/cw_orb.dart';
import '../../main.dart';
import '../../services/locator.dart';
import '../../api/ai_api.dart';
import '../../utils/secure_storage.dart';
import 'widgets/balance_widget.dart';
import 'widgets/receive_widget.dart';
import 'widgets/send_confirm_widget.dart';
import 'widgets/swap_confirm_widget.dart';
import 'widgets/tx_result_widget.dart';
import 'widgets/history_widget.dart';
import 'widgets/audit_widget.dart';
import 'widgets/token_info_widget.dart';
import 'widgets/clarify_widget.dart';
import 'widgets/session_list_sheet.dart';

// ---------------------------------------------------------------------------
// Message model
// ---------------------------------------------------------------------------

enum ChatMsgKind { user, ai, thinking, widget }

enum WidgetType { balance, receive, sendConfirm, swapConfirm, txResult, history, audit, clarify, tokenInfo }

class ChatMsg {
  final ChatMsgKind kind;
  String text;
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
  _Suggestion(zhText: '最近的交易记录', enText: 'Recent transactions'),
  _Suggestion(zhText: '安全审计', enText: 'Security audit'),
  _Suggestion(zhText: '我的收款地址', enText: 'Show my address'),
];

// ---------------------------------------------------------------------------
// ChatView
// ---------------------------------------------------------------------------

class ChatView extends StatefulWidget {
  const ChatView({super.key});

  @override
  State<ChatView> createState() => ChatViewState();
}

class ChatViewState extends State<ChatView> {
  final _controller = TextEditingController();
  final _scrollController = ScrollController();
  final _messages = <ChatMsg>[];
  final _focusNode = FocusNode();

  String? _sessionId;
  StreamSubscription? _streamSub;

  @override
  void dispose() {
    _controller.dispose();
    _scrollController.dispose();
    _focusNode.dispose();
    _streamSub?.cancel();
    super.dispose();
  }

  Lang get _lang => CowalletApp.of(context).lang;
  bool get _isEmpty => _messages.isEmpty;

  void sendMessage(String message) {
    _send(message);
  }

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

  /// Start a new topic
  void startNewSession() {
    setState(() {
      _sessionId = null;
      _messages.clear();
    });
  }

  /// Load a specific session
  Future<void> loadSession(String sessionId) async {
    final result = await AiApi.getSessionMessages(sessionId: sessionId);
    if (!mounted) return;

    if (result.isSuccess && result.data != null) {
      setState(() {
        _sessionId = sessionId;
        _messages.clear();
        for (final msg in result.data!) {
          final role = msg['role'] as String? ?? '';
          final content = msg['content'] as String? ?? '';
          if (role == 'user') {
            _messages.add(ChatMsg(kind: ChatMsgKind.user, text: content));
          } else if (role == 'assistant' && content.isNotEmpty) {
            _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: content));
          }
        }
      });
    }
  }

  // -- send logic (streaming) -----------------------------------------------

  void _send([String? override]) {
    final text = (override ?? _controller.text).trim();
    if (text.isEmpty) return;

    setState(() {
      _messages.add(ChatMsg(kind: ChatMsgKind.user, text: text));
      _controller.clear();
      // Add AI message placeholder for streaming
      _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: ''));
    });
    _scrollToBottom();

    final aiMsgIndex = _messages.length - 1;

    _doStream(text, aiMsgIndex);
  }

  Future<void> _doStream(String text, int aiMsgIndex) async {
    final walletAddress = CowalletApp.of(context).walletAddress;
    final userId = await SecureStorage.getUserId();

    // Build multi-chain portfolio context for AI
    final balanceService = Services.balance;
    final portfolioContext = <String, dynamic>{
      'total_usd': balanceService.portfolioTotalUsd,
      'chains': balanceService.chainTotals.entries.map((entry) {
        return {
          'chain_id': entry.key,
          'total_usd': entry.value,
        };
      }).toList(),
    };

    // Supported chains (EVM chains we support)
    const supportedChains = [
      1,     // Ethereum
      8453,  // Base
      42161, // Arbitrum
      10,    // Optimism
      56,    // BNB Chain
      137,   // Polygon
    ];

    final stream = AiApi.chatStream(
      message: text,
      sessionId: _sessionId,
      userId: userId,
      walletAddress: walletAddress.isNotEmpty ? walletAddress : null,
      supportedChains: supportedChains,
      portfolioContext: portfolioContext,
    );

    _streamSub?.cancel();
    _streamSub = stream.listen(
      (event) {
        if (!mounted) return;

        switch (event.event) {
          case 'session':
            _sessionId = event.data['session_id'] as String?;
            break;

          case 'token':
            final tokenText = event.data['text'] as String? ?? '';
            setState(() {
              _messages[aiMsgIndex].text += tokenText;
            });
            _scrollToBottom();
            break;

          case 'tool_call':
            // Tool calls with kind=write show confirmation widgets immediately
            final name = event.data['name'] as String? ?? '';
            final id = event.data['id'] as String? ?? '';
            final kind = event.data['kind'] as String? ?? 'read';
            final params = event.data['parameters'] as Map<String, dynamic>? ?? {};

            if (kind == 'write' && name == 'send_transaction') {
              setState(() {
                _messages.add(ChatMsg(
                  kind: ChatMsgKind.widget,
                  widgetType: WidgetType.sendConfirm,
                  widgetData: {
                    'to_address': params['to_address'] ?? '',
                    'amount': params['value'] ?? '0',
                    'token': params['token'] ?? 'ETH',
                  },
                  toolCallId: id,
                ));
              });
              _scrollToBottom();
            } else if (kind == 'write' && name == 'swap_token') {
              setState(() {
                _messages.add(ChatMsg(
                  kind: ChatMsgKind.widget,
                  widgetType: WidgetType.swapConfirm,
                  widgetData: {
                    'from_token': params['from_token'] ?? '',
                    'to_token': params['to_token'] ?? '',
                    'amount': params['amount'] ?? '0',
                    'slippage': params['slippage'] ?? 0.5,
                    'chain_id': params['chain_id'],
                  },
                  toolCallId: id,
                ));
              });
              _scrollToBottom();
            }
            break;

          case 'tool_result':
            final toolName = event.data['tool_name'] as String? ?? '';
            final kind = event.data['kind'] as String? ?? 'read';
            final widgetType = event.data['widget_type'] as String?;
            final success = event.data['success'] as bool? ?? false;
            final result = event.data['result'] as Map<String, dynamic>? ?? {};

            if (!success) break;

            // Meta tools (clarify) render directly
            if (kind == 'meta' && widgetType == 'clarify') {
              final question = result['question'] as String? ?? '';
              final options = (result['options'] as List<dynamic>?) ?? [];
              setState(() {
                _messages.add(ChatMsg(
                  kind: ChatMsgKind.widget,
                  widgetType: WidgetType.clarify,
                  widgetData: {'question': question, 'options': options},
                ));
              });
              _scrollToBottom();
              break;
            }

            // Write tools already rendered at tool_call phase
            if (kind == 'write') {
              // Update swap widget with estimated output
              if (toolName == 'swap_token') {
                final estimated = result['estimated_output'] as String?;
                if (estimated != null) {
                  setState(() {
                    // Find the last swap widget and update estimated output
                    for (int i = _messages.length - 1; i >= 0; i--) {
                      if (_messages[i].widgetType == WidgetType.swapConfirm && !_messages[i].confirmed) {
                        _messages[i].widgetData['estimated_output'] = estimated;
                        break;
                      }
                    }
                  });
                }
              }
              // Update send widget with gas estimate from backend
              if (toolName == 'send_transaction') {
                final gasEstimate = result['gas_estimate'] as Map<String, dynamic>?;
                if (gasEstimate != null) {
                  final costEth = gasEstimate['cost_eth'] as String? ?? '';
                  final costUsd = gasEstimate['cost_usd'] as String?;
                  String gasDisplay = '~$costEth ETH';
                  if (costUsd != null) {
                    gasDisplay += ' ($costUsd)';
                  }
                  setState(() {
                    for (int i = _messages.length - 1; i >= 0; i--) {
                      if (_messages[i].widgetType == WidgetType.sendConfirm && !_messages[i].confirmed) {
                        _messages[i].widgetData['gas_estimate'] = gasDisplay;
                        break;
                      }
                    }
                  });
                }
              }
              break;
            }

            // Read tools: render widget based on widget_type
            switch (widgetType ?? toolName) {
              case 'balance':
              case 'get_balance':
                setState(() {
                  _messages.add(ChatMsg(
                    kind: ChatMsgKind.widget,
                    widgetType: WidgetType.balance,
                    widgetData: result,
                  ));
                });
                _scrollToBottom();
                break;
              case 'receive':
              case 'get_wallet_address':
                final addr = result['address'] as String? ??
                    CowalletApp.of(context).walletAddress;
                if (addr.isNotEmpty) {
                  setState(() {
                    _messages.add(ChatMsg(
                      kind: ChatMsgKind.widget,
                      widgetType: WidgetType.receive,
                      widgetData: {'address': addr},
                    ));
                  });
                  _scrollToBottom();
                }
                break;
              case 'history':
              case 'get_transaction_history':
                final transactions = (result['transactions'] as List<dynamic>?) ?? [];
                final total = result['total'] as int? ?? transactions.length;
                setState(() {
                  _messages.add(ChatMsg(
                    kind: ChatMsgKind.widget,
                    widgetType: WidgetType.history,
                    widgetData: {'transactions': transactions, 'total': total},
                  ));
                });
                _scrollToBottom();
                break;
              case 'audit':
              case 'security_audit':
                setState(() {
                  _messages.add(ChatMsg(
                    kind: ChatMsgKind.widget,
                    widgetType: WidgetType.audit,
                    widgetData: result,
                  ));
                });
                _scrollToBottom();
                break;
              case 'token_info':
              case 'get_token_info':
                setState(() {
                  _messages.add(ChatMsg(
                    kind: ChatMsgKind.widget,
                    widgetType: WidgetType.tokenInfo,
                    widgetData: result,
                  ));
                });
                _scrollToBottom();
                break;
            }
            break;

          case 'done':
            // Remove empty AI message if no text was streamed
            setState(() {
              if (_messages[aiMsgIndex].text.isEmpty) {
                _messages.removeAt(aiMsgIndex);
              }
            });
            break;

          case 'error':
            setState(() {
              _messages[aiMsgIndex].text =
                  event.data['message'] as String? ?? '请求失败，请稍后重试';
            });
            break;
        }
      },
      onError: (e) {
        if (!mounted) return;
        setState(() {
          _messages[aiMsgIndex].text = '网络错误，请稍后重试';
        });
      },
    );
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
        _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: result.message));
      } else {
        _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: '⚠ ${result.message}'));
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

  Future<void> _onSwapConfirm(int index) async {
    final msg = _messages[index];
    setState(() => msg.loading = true);

    final params = {
      'from_token': msg.widgetData['from_token'] as String? ?? '',
      'to_token': msg.widgetData['to_token'] as String? ?? '',
      'amount': msg.widgetData['amount'] as String? ?? '0',
    };

    final result = await Services.intent.execute('swap', params);

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
            'token': '${params['from_token']} → ${params['to_token']}',
          },
        ));
        _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: result.message));
      } else {
        _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: '⚠ ${result.message}'));
      }
    });
    _scrollToBottom();
  }

  void _onSwapDeny(int index) {
    setState(() {
      _messages[index].confirmed = true;
      _messages.add(ChatMsg(kind: ChatMsgKind.ai, text: '好的，已取消兑换。'));
    });
    _scrollToBottom();
  }

  void _onClarifySelect(int index, String prompt) {
    setState(() {
      _messages[index].confirmed = true;
    });
    _send(prompt);
  }

  // -- build -----------------------------------------------------------------

  void _showSessionHistory() {
    showModalBottomSheet(
      context: context,
      isScrollControlled: true,
      backgroundColor: Colors.transparent,
      builder: (_) => SessionListSheet(
        onSessionTap: (sessionId) => loadSession(sessionId),
        onNewChat: () => startNewSession(),
      ),
    );
  }

  // -- build -----------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      bottom: false,
      child: Column(
        children: [
          _buildHeader(),
          Expanded(child: _isEmpty ? _buildEmptyState() : _buildConversation()),
          _buildComposer(),
        ],
      ),
    );
  }

  Widget _buildHeader() {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
      decoration: const BoxDecoration(
        color: CwColors.bgPaper,
        border: Border(bottom: BorderSide(color: CwColors.line)),
      ),
      child: Row(
        children: [
          GestureDetector(
            onTap: _showSessionHistory,
            child: Container(
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: CwColors.bgSubtle,
                borderRadius: BorderRadius.circular(8),
              ),
              child: const Icon(Icons.history_rounded, size: 20, color: CwColors.ink2),
            ),
          ),
          const Spacer(),
          Text(
            S.askCowallet,
            style: const TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w600,
              color: CwColors.ink1,
            ),
          ),
          const Spacer(),
          GestureDetector(
            onTap: startNewSession,
            child: Container(
              padding: const EdgeInsets.all(8),
              decoration: BoxDecoration(
                color: CwColors.bgSubtle,
                borderRadius: BorderRadius.circular(8),
              ),
              child: const Icon(Icons.add_rounded, size: 20, color: CwColors.ink2),
            ),
          ),
        ],
      ),
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
            msg.text.isEmpty
                ? _ThinkingDots()
                : MarkdownBody(
                    data: msg.text,
                    shrinkWrap: true,
                    selectable: true,
                    styleSheet: MarkdownStyleSheet(
                      p: const TextStyle(fontSize: 14, height: 1.55, color: CwColors.ink1),
                      strong: const TextStyle(fontSize: 14, height: 1.55, color: CwColors.ink1, fontWeight: FontWeight.w600),
                      em: const TextStyle(fontSize: 14, height: 1.55, color: CwColors.ink1, fontStyle: FontStyle.italic),
                      h1: const TextStyle(fontSize: 20, height: 1.4, color: CwColors.ink1, fontWeight: FontWeight.w700),
                      h2: const TextStyle(fontSize: 18, height: 1.4, color: CwColors.ink1, fontWeight: FontWeight.w700),
                      h3: const TextStyle(fontSize: 16, height: 1.4, color: CwColors.ink1, fontWeight: FontWeight.w600),
                      h4: const TextStyle(fontSize: 15, height: 1.4, color: CwColors.ink1, fontWeight: FontWeight.w600),
                      code: TextStyle(
                        fontFamily: 'JetBrainsMono',
                        fontSize: 13,
                        color: CwColors.ink1,
                        backgroundColor: CwColors.bgCard,
                      ),
                      codeblockDecoration: BoxDecoration(
                        color: CwColors.bgCard,
                        borderRadius: BorderRadius.circular(8),
                        border: Border.all(color: CwColors.line),
                      ),
                      codeblockPadding: const EdgeInsets.all(12),
                      a: const TextStyle(fontSize: 14, height: 1.55, color: CwColors.accent, decoration: TextDecoration.none),
                      listBullet: const TextStyle(fontSize: 14, height: 1.55, color: CwColors.ink1),
                      listIndent: 20.0,
                      blockSpacing: 10.0,
                      pPadding: EdgeInsets.zero,
                      h1Padding: EdgeInsets.zero,
                      h2Padding: EdgeInsets.zero,
                      h3Padding: EdgeInsets.zero,
                      h4Padding: EdgeInsets.zero,
                      h5Padding: EdgeInsets.zero,
                      h6Padding: EdgeInsets.zero,
                    ),
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
      case WidgetType.swapConfirm:
        return ChatSwapConfirmWidget(
          fromToken: msg.widgetData['from_token'] ?? '',
          toToken: msg.widgetData['to_token'] ?? '',
          amount: msg.widgetData['amount'] ?? '0',
          estimatedOutput: msg.widgetData['estimated_output'] ?? '—',
          slippage: (msg.widgetData['slippage'] as num?)?.toDouble() ?? 0.5,
          chainId: msg.widgetData['chain_id'] as int?,
          loading: msg.loading,
          resolved: msg.confirmed,
          onConfirm: () => _onSwapConfirm(index),
          onDeny: () => _onSwapDeny(index),
        );
      case WidgetType.txResult:
        return ChatTxResultWidget(
          txHash: msg.widgetData['tx_hash'] ?? '',
          success: msg.widgetData['success'] ?? true,
          amount: msg.widgetData['amount'],
          token: msg.widgetData['token'],
        );
      case WidgetType.history:
        return ChatHistoryWidget(
          transactions: (msg.widgetData['transactions'] as List<dynamic>?) ?? [],
          total: msg.widgetData['total'] as int? ?? 0,
        );
      case WidgetType.audit:
        return ChatAuditWidget(data: msg.widgetData);
      case WidgetType.tokenInfo:
        return ChatTokenInfoWidget(data: msg.widgetData);
      case WidgetType.clarify:
        final options = (msg.widgetData['options'] as List<dynamic>? ?? [])
            .map((o) => ClarifyOption.fromJson(o is Map<String, dynamic> ? o : {}))
            .toList();
        return ChatClarifyWidget(
          question: msg.widgetData['question'] ?? '',
          options: options,
          resolved: msg.confirmed,
          onSelect: (prompt) => _onClarifySelect(index, prompt),
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
// Claude Code-style thinking indicator
// ---------------------------------------------------------------------------

class _ThinkingDots extends StatefulWidget {
  @override
  State<_ThinkingDots> createState() => _ThinkingDotsState();
}

class _ThinkingDotsState extends State<_ThinkingDots>
    with SingleTickerProviderStateMixin {
  late AnimationController _ctrl;
  int _phraseIndex = 0;

  static const _phrases = ['思考中', '分析中', '处理中', '理解中'];

  @override
  void initState() {
    super.initState();
    _ctrl = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 2000),
    )..repeat();
    _ctrl.addStatusListener((status) {
      if (status == AnimationStatus.completed) {
        setState(() => _phraseIndex = (_phraseIndex + 1) % _phrases.length);
      }
    });
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
          children: [
            _buildPulsingOrb(),
            const SizedBox(width: 10),
            _buildShimmerText(),
          ],
        );
      },
    );
  }

  Widget _buildPulsingOrb() {
    final scale = 1.0 + 0.2 * _sin(_ctrl.value * 3.14159 * 2);
    final opacity = 0.6 + 0.4 * _sin(_ctrl.value * 3.14159 * 2);
    return Transform.scale(
      scale: scale,
      child: Opacity(
        opacity: opacity,
        child: Container(
          width: 8, height: 8,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            gradient: RadialGradient(
              colors: [
                CwColors.accent.withValues(alpha: 0.9),
                CwColors.accent.withValues(alpha: 0.3),
              ],
            ),
          ),
        ),
      ),
    );
  }

  Widget _buildShimmerText() {
    final shimmerPosition = _ctrl.value;
    return ShaderMask(
      shaderCallback: (bounds) {
        return LinearGradient(
          begin: Alignment(-1.0 + 2.0 * shimmerPosition, 0),
          end: Alignment(0.0 + 2.0 * shimmerPosition, 0),
          colors: const [
            CwColors.ink4,
            CwColors.ink2,
            CwColors.ink4,
          ],
          stops: const [0.0, 0.5, 1.0],
        ).createShader(bounds);
      },
      child: Text(
        _phrases[_phraseIndex],
        style: const TextStyle(
          fontSize: 13,
          fontWeight: FontWeight.w500,
          color: Colors.white,
          letterSpacing: 0.5,
        ),
      ),
    );
  }

  double _sin(double x) {
    final n = x % 3.14159;
    return 4 * n * (3.14159 - n) / (3.14159 * 3.14159);
  }
}
