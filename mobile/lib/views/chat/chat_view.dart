import 'package:flutter/material.dart';
import '../../theme/colors.dart';
import '../../l10n/strings.dart';
import '../../widgets/cw_orb.dart';
import '../../widgets/intent_card.dart';
import '../../main.dart';
import '../../services/locator.dart';

// ---------------------------------------------------------------------------
// Intent recognition engine
// ---------------------------------------------------------------------------

typedef ParamExtractor = Map<String, String> Function(
    RegExpMatch match, Lang lang);

class IntentRule {
  final RegExp reZh;
  final RegExp reEn;
  final String kind;
  final String titleZh;
  final String titleEn;
  final String subZh;
  final String subEn;
  final String yesZh;
  final String yesEn;
  final String noZh;
  final String noEn;
  final ParamExtractor? extractParams;
  final String Function(Lang lang, Map<String, String> data)? dynamicTitle;
  final String Function(Lang lang, Map<String, String> data)? dynamicSub;

  IntentRule({
    required this.reZh,
    required this.reEn,
    required this.kind,
    required this.titleZh,
    required this.titleEn,
    required this.subZh,
    required this.subEn,
    required this.yesZh,
    required this.yesEn,
    required this.noZh,
    required this.noEn,
    this.extractParams,
    this.dynamicTitle,
    this.dynamicSub,
  });

  String title(Lang lang, [Map<String, String>? data]) =>
      (data != null && dynamicTitle != null)
          ? dynamicTitle!(lang, data)
          : lang == Lang.zh
              ? titleZh
              : titleEn;
  String sub(Lang lang, [Map<String, String>? data]) =>
      (data != null && dynamicSub != null)
          ? dynamicSub!(lang, data)
          : lang == Lang.zh
              ? subZh
              : subEn;
  String yes(Lang lang) => lang == Lang.zh ? yesZh : yesEn;
  String no(Lang lang) => lang == Lang.zh ? noZh : noEn;
}

final List<IntentRule> _intentRules = [
  IntentRule(
    reZh: RegExp(r'(闲着|闲放|放哪|存起来|赚利息|生息|理财|没用)'),
    reEn: RegExp(r'(idle|sitting|park|save|earn interest|yield)', caseSensitive: false),
    kind: 'savings',
    titleZh: '把闲钱放去赚利息',
    titleEn: 'Put idle funds to work',
    subZh: '给你闲着的钱找个赚利息的地方',
    subEn: 'Find a place for your idle money to earn interest',
    yesZh: '好,帮我看看',
    yesEn: 'Yes, show me',
    noZh: '不是这个意思',
    noEn: "That's not what I meant",
  ),
  IntentRule(
    reZh: RegExp(r'(?:转|发|send)\s*([\d.]+)\s*(ETH|USDC|eth|usdc)\s*(?:到|给|to)\s*(0x[a-fA-F0-9]{40})'),
    reEn: RegExp(r'(?:send|transfer)\s*([\d.]+)\s*(ETH|USDC|eth|usdc)\s*to\s*(0x[a-fA-F0-9]{40})', caseSensitive: false),
    kind: 'transfer',
    titleZh: '转账确认',
    titleEn: 'Transfer confirmation',
    subZh: '正在准备…',
    subEn: 'Preparing…',
    yesZh: '确认转账',
    yesEn: 'Confirm transfer',
    noZh: '取消',
    noEn: 'Cancel',
    extractParams: (match, lang) => {
          'amount': match.group(1) ?? '0',
          'token': (match.group(2) ?? 'ETH').toUpperCase(),
          'to': match.group(3) ?? '',
        },
    dynamicTitle: (lang, data) {
      final amount = data['amount'] ?? '0';
      final token = data['token'] ?? 'ETH';
      final to = data['to'] ?? '';
      final shortTo = to.length >= 10
          ? '${to.substring(0, 6)}...${to.substring(to.length - 4)}'
          : to;
      return lang == Lang.zh
          ? '转 $amount $token 到 $shortTo'
          : 'Send $amount $token to $shortTo';
    },
    dynamicSub: (lang, data) {
      final gas = data['gas'];
      if (gas != null) {
        return lang == Lang.zh ? '预估 Gas: ~$gas' : 'Est. gas: ~$gas';
      }
      return lang == Lang.zh ? '正在估算 Gas…' : 'Estimating gas…';
    },
  ),
  IntentRule(
    reZh: RegExp(r'(老婆|妻子|生日|Sarah|sarah)'),
    reEn: RegExp(r'(wife|sarah|birthday)', caseSensitive: false),
    kind: 'transfer_demo',
    titleZh: '给 Sarah (你老婆) 转 \$1000 USDC',
    titleEn: 'Send \$1000 USDC to Sarah (your wife)',
    subZh: '从你的主钱包转到 Sarah 的地址',
    subEn: 'From your main wallet to Sarah\'s address',
    yesZh: '确认转账',
    yesEn: 'Confirm transfer',
    noZh: '不对,取消',
    noEn: 'No, cancel',
  ),
  IntentRule(
    reZh: RegExp(r'(买点?苹果|买苹果|苹果股)'),
    reEn: RegExp(r'(buy apple|apple stock|appl|aapl)', caseSensitive: false),
    kind: 'apple',
    titleZh: '买入苹果公司股票代币 (AAPL)',
    titleEn: 'Buy Apple stock token (AAPL)',
    subZh: '链上证券代币,实时结算',
    subEn: 'On-chain security token, real-time settlement',
    yesZh: '买入',
    yesEn: 'Buy',
    noZh: '先不买',
    noEn: 'Not now',
  ),
  IntentRule(
    reZh: RegExp(r'(花了多少|这个月.*花|支出|开销|花销)'),
    reEn: RegExp(r'(how much.*spen|this month.*spen|expense|spending)', caseSensitive: false),
    kind: 'spending',
    titleZh: '你这个月花了 \$2,847',
    titleEn: 'You spent \$2,847 this month',
    subZh: '餐饮 \$820 · 订阅 \$340 · 购物 \$1,687',
    subEn: 'Dining \$820 · Subs \$340 · Shopping \$1,687',
    yesZh: '看详细账单',
    yesEn: 'See full breakdown',
    noZh: '知道了',
    noEn: 'Got it',
  ),
  IntentRule(
    reZh: RegExp(r'(余额|总共.*多少|放哪|钱都|资产)'),
    reEn: RegExp(r'(balance|total|where.*money|how much.*have)', caseSensitive: false),
    kind: 'balance',
    titleZh: '正在查询余额…',
    titleEn: 'Checking balance…',
    subZh: '请稍候',
    subEn: 'Please wait',
    yesZh: '看全部资产',
    yesEn: 'See all assets',
    noZh: '好的',
    noEn: 'OK',
    dynamicTitle: (lang, data) {
      final total = data['total'] ?? '—';
      return lang == Lang.zh ? '总共 $total' : 'Total $total';
    },
    dynamicSub: (lang, data) {
      final eth = data['eth'] ?? '—';
      final usdc = data['usdc'] ?? '—';
      return '$eth · $usdc';
    },
  ),
];

({IntentRule rule, Map<String, String> params})? detectIntent(
    String text, Lang lang) {
  for (final rule in _intentRules) {
    final re = lang == Lang.zh ? rule.reZh : rule.reEn;
    final match = re.firstMatch(text);
    if (match != null) {
      final params = rule.extractParams?.call(match, lang) ?? {};
      return (rule: rule, params: params);
    }
  }
  return null;
}

// ---------------------------------------------------------------------------
// Message model
// ---------------------------------------------------------------------------

enum _MsgKind { user, ai, thinking, intentCard }

class _ChatMsg {
  final _MsgKind kind;
  final String text;
  final IntentRule? intent;
  final Map<String, String> intentParams;
  bool intentResolved = false;
  bool intentLoading = false;
  Map<String, String>? resultData;

  _ChatMsg({
    required this.kind,
    this.text = '',
    this.intent,
    this.intentParams = const {},
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
  _Suggestion(zhText: '我这个月花了多少?', enText: 'How much did I spend?'),
  _Suggestion(zhText: '闲钱放哪赚利息', enText: 'Where to park idle funds'),
  _Suggestion(zhText: '我的余额是多少', enText: 'What\'s my balance'),
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
  final _messages = <_ChatMsg>[];
  final _focusNode = FocusNode();

  @override
  void dispose() {
    _controller.dispose();
    _scrollController.dispose();
    _focusNode.dispose();
    super.dispose();
  }

  // -- helpers ---------------------------------------------------------------

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

  void _send([String? override]) {
    final text = (override ?? _controller.text).trim();
    if (text.isEmpty) return;

    setState(() {
      _messages.add(_ChatMsg(kind: _MsgKind.user, text: text));
      _controller.clear();
    });
    _scrollToBottom();

    // Show thinking dots
    setState(() {
      _messages.add(_ChatMsg(kind: _MsgKind.thinking));
    });
    _scrollToBottom();

    // After delay, resolve intent
    Future.delayed(const Duration(milliseconds: 800), () async {
      if (!mounted) return;
      final detected = detectIntent(text, _lang);

      setState(() {
        _messages.removeWhere((m) => m.kind == _MsgKind.thinking);

        if (detected != null) {
          _messages.add(_ChatMsg(kind: _MsgKind.ai, text: S.intentConfirming));
          _messages.add(_ChatMsg(
            kind: _MsgKind.intentCard,
            intent: detected.rule,
            intentParams: detected.params,
          ));
        } else {
          _messages.add(_ChatMsg(
            kind: _MsgKind.ai,
            text: _lang == Lang.zh
                ? '我没太听懂你想做什么。试试换个说法?'
                : "I didn't quite catch that. Try rephrasing?",
          ));
        }
      });
      _scrollToBottom();

      // Pre-fetch gas estimate for transfer intents
      if (detected != null && detected.rule.kind == 'transfer') {
        final cardIndex = _messages.length - 1;
        final gasResult = await Services.intent.estimateTransferGas(detected.params);
        if (!mounted) return;
        if (gasResult.success) {
          setState(() {
            _messages[cardIndex].resultData = {
              ...detected.params,
              ...gasResult.data,
            };
          });
        }
      }
    });
  }

  Future<void> _onIntentConfirm(int index) async {
    final msg = _messages[index];
    setState(() {
      msg.intentLoading = true;
    });

    final result = await Services.intent.execute(
      msg.intent!.kind,
      msg.intentParams,
    );

    if (!mounted) return;
    setState(() {
      msg.intentLoading = false;
      msg.intentResolved = true;
      if (result.success && result.data.isNotEmpty) {
        msg.resultData = result.data;
      }
      _messages.add(_ChatMsg(
        kind: _MsgKind.ai,
        text: result.success ? result.message : '⚠ ${result.message}',
      ));
    });
    _scrollToBottom();
  }

  void _onIntentDeny(int index) {
    setState(() {
      _messages[index].intentResolved = true;
      _messages.add(_ChatMsg(kind: _MsgKind.ai, text: S.intentMisread));
    });
    _scrollToBottom();
  }

  // -- build -----------------------------------------------------------------

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Expanded(
          child: _isEmpty ? _buildEmptyState() : _buildConversation(),
        ),
        _buildComposer(),
      ],
    );
  }

  // -- empty state -----------------------------------------------------------

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
          style: const TextStyle(
            fontSize: 13,
            color: CwColors.ink2,
            height: 1.3,
          ),
        ),
      ),
    );
  }

  // -- conversation ----------------------------------------------------------

  Widget _buildConversation() {
    return ListView.builder(
      controller: _scrollController,
      padding: const EdgeInsets.fromLTRB(20, 16, 20, 8),
      itemCount: _messages.length,
      itemBuilder: (_, i) {
        final msg = _messages[i];
        switch (msg.kind) {
          case _MsgKind.user:
            return _buildUserBubble(msg);
          case _MsgKind.ai:
            return _buildAiMessage(msg);
          case _MsgKind.thinking:
            return _buildThinkingDots();
          case _MsgKind.intentCard:
            return _buildIntentCard(msg, i);
        }
      },
    );
  }

  // -- user bubble -----------------------------------------------------------

  Widget _buildUserBubble(_ChatMsg msg) {
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
          style: const TextStyle(
            fontSize: 14,
            height: 1.5,
            color: Colors.white,
          ),
        ),
      ),
    );
  }

  // -- AI message ------------------------------------------------------------

  Widget _buildAiMessage(_ChatMsg msg) {
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
            // "who" label with orb dot
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 6,
                  height: 6,
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
            // Body text
            Text(
              msg.text,
              style: const TextStyle(
                fontSize: 14,
                height: 1.55,
                color: CwColors.ink1,
              ),
            ),
          ],
        ),
      ),
    );
  }

  // -- thinking dots ---------------------------------------------------------

  Widget _buildThinkingDots() {
    return Align(
      alignment: Alignment.centerLeft,
      child: Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: _ThinkingDots(),
      ),
    );
  }

  // -- intent card -----------------------------------------------------------

  Widget _buildIntentCard(_ChatMsg msg, int index) {
    final intent = msg.intent!;
    final lang = _lang;
    final displayData = msg.resultData ?? {};
    return Align(
      alignment: Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.only(bottom: 12),
        constraints: BoxConstraints(
          maxWidth: MediaQuery.of(context).size.width * 0.85,
        ),
        child: IntentCard(
          headerLabel: S.intentHeader,
          title: intent.title(lang, displayData),
          subtitle: intent.sub(lang, displayData),
          confirmLabel: intent.yes(lang),
          denyLabel: intent.no(lang),
          loading: msg.intentLoading,
          onConfirm:
              (msg.intentResolved || msg.intentLoading)
                  ? null
                  : () => _onIntentConfirm(index),
          onDeny:
              (msg.intentResolved || msg.intentLoading)
                  ? null
                  : () => _onIntentDeny(index),
        ),
      ),
    );
  }

  // -- composer --------------------------------------------------------------

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
            // Camera / image upload
            IconButton(
              icon: const Icon(Icons.camera_alt_outlined, size: 22),
              color: CwColors.ink3,
              onPressed: () {
                // Image upload placeholder
              },
            ),
            // Text field
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
                    horizontal: 16,
                    vertical: 10,
                  ),
                  isDense: true,
                ),
                textInputAction: TextInputAction.send,
                onSubmitted: (_) => _send(),
              ),
            ),
            // Microphone
            IconButton(
              icon: const Icon(Icons.mic_none_rounded, size: 22),
              color: CwColors.ink3,
              onPressed: () {
                // Voice input placeholder
              },
            ),
            // Send
            SizedBox(
              width: 36,
              height: 36,
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
      builder: (_, _) {
        return Row(
          mainAxisSize: MainAxisSize.min,
          children: List.generate(3, (i) {
            // Stagger each dot by 0.2
            final delay = i * 0.2;
            final t = (_ctrl.value - delay).clamp(0.0, 1.0);
            // Sine wave for bounce + fade
            final progress = (t * 3.14159).clamp(0.0, 3.14159);
            final sinVal = _sin(progress);
            final opacity = 0.3 + 0.7 * sinVal;
            final offset = -3.0 * sinVal;

            return Padding(
              padding: EdgeInsets.only(
                right: i < 2 ? 4 : 0,
              ),
              child: Transform.translate(
                offset: Offset(0, offset),
                child: Opacity(
                  opacity: opacity,
                  child: Container(
                    width: 6,
                    height: 6,
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

  // Minimal sine approximation to avoid importing dart:math for just this
  double _sin(double x) {
    // Normalize to [0, pi]
    final n = x % 3.14159;
    // Quadratic approximation: 4n(pi-n)/pi^2
    return 4 * n * (3.14159 - n) / (3.14159 * 3.14159);
  }
}
