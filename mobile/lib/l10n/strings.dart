enum Lang { zh, en }

class S {
  S._();

  static Lang _lang = Lang.zh;
  static Lang get lang => _lang;
  static void setLang(Lang l) => _lang = l;

  static String _p(String zh, String en) => _lang == Lang.zh ? zh : en;

  // App
  static String get appName => 'cowallet';
  static String get tagline => _p('会听懂人话的钱包', 'the wallet that reads you back');

  // Tabs
  static String get tabHome => _p('首页', 'Home');
  static String get tabWallet => _p('钱包', 'Wallet');
  static String get tabAsk => _p('问', 'ASK');
  static String get tabAgents => _p('助手', 'Agents');
  static String get tabSettings => _p('设置', 'Settings');

  // Onboarding — Hero
  static String get heroKicker => _p('数字钱包 · 会听懂人话', 'Digital wallet · speaks your language');
  static String get heroH1a => _p('会听你说话的', 'A wallet that');
  static String get heroH1b => _p('', 'actually');
  static String get heroH1em => _p('钱包', 'listens');
  static String get heroExplain => _p(
    '就像给你家请了个管家——你说"帮我转 100 块给小明",它就去做;你不会说也没关系,它有按钮。',
    'Like hiring a butler for your money — say "send \$100 to Sarah" and it does it. Don\'t feel like talking? Buttons work too.',
  );
  static String get heroFeat1h => _p('只有你能动你的钱', 'Only you can move your money');
  static String get heroFeat1s => _p('我们也进不来', 'Not even us');
  static String get heroFeat2h => _p('100+ 个金融网络', '100+ financial networks');
  static String get heroFeat2s => _p('全世界通用', 'Works worldwide');
  static String get heroFeat3h => _p('AI 帮你跑腿', 'AI does the errands');
  static String get heroFeat3s => _p('你只需说一句话', 'Just say the word');
  static String get getStarted => _p('开始使用', 'Get started');
  static String get haveWallet => _p('我已经有钱包了', 'I already have a wallet');
  static String get heroLegal => _p('继续即表示同意服务条款与隐私政策', 'By continuing you agree to our Terms and Privacy Policy');

  // Onboarding — Start
  static String get startH1 => _p('这是你的第一个钱包吗?', 'Is this your first wallet?');
  static String get startSub => _p('两分钟就能开好。选一个最适合你的方式。', 'Takes two minutes. Pick what fits you.');
  static String get pickCreateTitle => _p('我是新用户,帮我开一个', "I'm new — set one up for me");
  static String get pickCreateDesc => _p('最简单。我们帮你把钥匙安全保管在三个地方。不用背一长串词。', 'Easiest. We split the key safely across three places. No long phrases to memorize.');
  static String get pickCreateTag => _p('推荐', 'Best');
  static String get pickImportTitle => _p('我已经有一个钱包', 'I have a wallet already');
  static String get pickImportDesc => _p('用那 12 或 24 个单词的"找回密码"把它搬进来。', 'Bring it in using those 12 or 24 recovery words.');
  static String get pickHwTitle => _p('我有一个硬件钱包', 'I have a hardware wallet');
  static String get pickHwDesc => _p('Ledger、Trezor 之类的实物钥匙。', 'Ledger, Trezor — physical key devices.');

  // Onboarding — Creating
  static String get creatingH1 => _p('正在帮你把钥匙分成三份', 'Splitting your key into three pieces');
  static String get creatingSub => _p('就像一份保险,任何一份丢了,剩下两份还能开门。完整的钥匙从不出现过。', 'Like an insurance policy — if one piece is lost, the other two still open the door. The whole key never exists in one place.');
  static String get cl1 => _p('这台手机里存一份', 'One piece on this phone');
  static String get cl2 => _p('云端保险柜存一份', 'One piece in the cloud safe');
  static String get cl3 => _p('找回凭证存在你这儿(稍后设置)', 'Recovery piece in your hands (set up later)');
  static String get createError => _p('钱包创建失败,请重试。', 'Wallet creation failed. Please try again.');
  static String get retry => _p('重试', 'Retry');

  // Onboarding — Importing
  static String get importH1 => _p('输入你的 12 或 24 个"找回词"', 'Enter your 12 or 24 recovery words');
  static String get importSub => _p('就是当初创建钱包时写下来的那一串单词。用空格分开。', 'The words you wrote down when you first set up your wallet. Separated by spaces.');
  static String get importWarn => _p('永远不要在网页上输入这些词', 'Never enter these on a website');
  static String get importWarnBody => _p(
    'cowallet 只在这个输入框里问你要。任何客服、空投网站都不会问。看到就是骗子。',
    'cowallet only asks here. No support rep, no airdrop site will ever ask. If they do, it\'s a scam.',
  );
  static String get importPlaceholder => _p('一个词 一个词 空格隔开…', 'word word word word…');
  static String get paste => _p('粘贴', 'Paste');
  static String get importSubmit => _p('导入我的钱包', 'Import my wallet');

  // Onboarding — Bio
  static String get bioH1 => _p('每次花钱用人脸确认', 'Confirm with your face');
  static String get bioSub => _p('就像手机解锁一样。你的脸不会离开这台手机。', 'Just like unlocking your phone. Your face never leaves this device.');
  static String get bioActivate => _p('打开人脸识别', 'Turn on Face ID');
  static String get bioSkip => _p('改用密码', 'Use a passcode instead');
  static String get bioDone => _p('Face ID 已开启', 'Face ID ready');

  // Onboarding — Name
  static String get nameH1 => _p('我该怎么叫你?', 'What should I call you?');
  static String get nameSub => _p('起个名字就行,不用真名。', 'A nickname works. No real name needed.');
  static String get namePlaceholder => _p('比如 小明 / 老王 / Alice', 'e.g. Alice, Mike, or a nickname');
  static String get nameHint => _p('存在你手机里,不会上传。', 'Stays on your phone. Never uploaded.');
  static String get continueBtn => _p('下一步', 'Continue');

  // Onboarding — Ready
  static String get readyH1 => _p('都搞定了。', 'All set.');
  static String readyH1Named(String name) => _p('都搞定了,$name。', "You're in, $name.");
  static String get readySub => _p('你的钱包已经可以用了。', 'Your wallet is ready to use.');
  static String get readyWhat => _p('接下来可以做这些', 'What you can do next');
  static String get ready1h => _p('收到的第一笔钱,gas 我们付', 'First incoming? Gas is on us');
  static String get ready1s => _p('给你试试手,不花你一分钱', 'Try it out, nothing from your pocket');
  static String get ready2h => _p('试着对它说话', 'Try talking to it');
  static String get ready2s => _p('"我的钱都放哪了" "最近有什么花销"', '"Show me my money" "How much did I spend this month?"');
  static String get ready3h => _p('7 天内设置找回凭证', 'Set up your recovery within 7 days');
  static String get ready3s => _p('防止换手机或丢手机', 'In case you lose your phone');
  static String get readyGo => _p('好,开始用', "OK, let's go");

  // Onboarding — Persona
  static String get personaH1 => _p('你主要想用它做什么?', 'What will you mostly use it for?');
  static String get personaSub => _p('我们给你布置一个最顺手的首页。以后可以改。', "We'll set up a home that fits you. You can switch later.");
  static String get personaDaily => _p('日常过日子', 'Everyday life');
  static String get personaDailyDesc => _p('收钱、转账、买点东西、存点利息。', 'Send money, receive, shop, earn some interest.');
  static String get personaDailyTag => _p('最受欢迎', 'Most popular');
  static String get personaTrader => _p('炒币 / 理财', 'Trading / Yield');
  static String get personaTraderDesc => _p('我会交易、追收益、让 AI 帮我盯盘。', 'I trade, chase yield, let AI watch the market.');
  static String get personaFamily => _p('家庭共管', 'Family account');
  static String get personaFamilyDesc => _p('家人帮我确认大额,但钱由我主控。', "Family helps confirm big moves, but the money's mine.");
  static String get personaFamilyTag => _p('新', 'New');
  static String get personaBuilder => _p('我做 AI / 开发', 'I build AI / apps');
  static String get personaBuilderDesc => _p('给我 API、skills 接口、测试环境。', 'Give me APIs, skills, a test sandbox.');
  static String get personaSkip => _p('先跳过,我再看看', 'Skip for now');

  // Navigation
  static String get back => _p('返回', 'Back');
  static String get skip => _p('跳过', 'Skip');

  // Home
  static String get homeStatus => _p('一切正常 · 三份钥匙都在 · 今天还没可疑动作', 'All good · all three key pieces safe · no odd activity today');
  static String get homeGreetMorning => _p('早上好,', 'Good morning,');
  static String get homeSlogan => _p('会听懂人话的钱包 —— 你说一句,它就能帮你办。', "A wallet that speaks your language — say what you need, it'll do it.");
  static String get yourTotal => _p('你的总资产', 'Your total');
  static String get send => _p('发送', 'Send');
  static String get receive => _p('收款', 'Receive');
  static String get scan => _p('扫码', 'Scan');
  static String get people => _p('联系人', 'People');
  static String get tryTalking => _p('试试跟它说话', 'Try talking to it');
  static String get recentActivity => _p('最近发生的事', 'Recent activity');
  static String get seeAll => _p('全部', 'See all');
  static String get onlyCowallet => _p('只有 cowallet 有的', 'Only cowallet does this');
  static String get today => _p('今天', 'today');

  // Home — Try prompts
  static String get try1h => _p('我这个月花了多少钱?', 'How much did I spend this month?');
  static String get try1s => _p('分类统计、省钱建议', 'Categorized, with tips to save');
  static String get try2h => _p('我那 5 万块闲着呢', 'I have \$50k just sitting');
  static String get try2s => _p('看看放哪能赚利息', 'See where it could earn interest');
  static String get try3h => _p('老婆生日,给她转 1000 块', "Wife's birthday, send her \$1000");
  static String get try3s => _p('联系人式转账', 'Transfer by contact name');

  // Home — Activity
  static String get actRecv => _p('收到 0.5 ETH', 'Received 0.5 ETH');
  static String get actRecvSub => _p('来自 朋友小明 · 2 小时前', 'From Xiao Ming · 2h ago');
  static String get actAi => _p('Claude 查了你的余额', 'Claude checked your balance');
  static String get actAiSub => _p('按你定的规矩,只读不动 · 4 小时前', 'Read-only, per your rules · 4h ago');
  static String get actPay => _p('自动付了 \$42 给 Figma', 'Auto-paid \$42 to Figma');
  static String get actPaySub => _p('订阅续费 · 昨天', 'Subscription renewal · yesterday');
  static String get actBlock => _p('拦下一个可疑授权', 'Blocked a suspicious approval');
  static String get actBlockSub => _p('对方想无限额度提款 · 2 天前', 'Wanted unlimited withdraw · 2d ago');

  // Home — Showcase
  static String get scAiReads => _p('AI 看懂交易', 'AI reads the contract');
  static String get scAiReadsSub => _p('签字前用人话告诉你', 'Explains in plain words');
  static String get scAgentPay => _p('让 AI 替你付款', 'Let AI pay for you');
  static String get scAgentPaySub => _p('订阅、账单自动跑', 'Subs and bills, hands-free');
  static String get scFamily => _p('家人共管', 'Family guard');
  static String get scFamilySub => _p('大额让家人再点一下', 'Big moves, family confirms');
  static String get scSkills => _p('接入 AI 助手', 'Connect AI agents');
  static String get scSkillsSub => _p('Claude、ChatGPT 一键插', 'Claude, ChatGPT — one tap');

  // Wallet
  static String get totalBalance => _p('总资产', 'Total balance');
  static String get swap => _p('兑换', 'Swap');
  static String get yourMoney => _p('你的资产', 'Your money');
  static String get securities => _p('证券代币 · 可选', 'Securities · optional');
  static String get securitiesNew => _p('新', 'New');
  static String get securitiesIntro => _p('美债、美股、黄金的链上版本。打开就能买。', 'Tokenized T-bills, stocks, and gold. One-tap to buy.');
  static String get browseAll => _p('看全部证券代币', 'Browse all');
  static String get earning => _p('在赚利息的钱', 'Earning money');

  // Chat
  static String get chatEmpty => _p('说点什么?', "What's on your mind?");
  static String get chatEmptySub => _p('说话、打字、拍张照都行。', 'Talk, type, or send a photo.');
  static String get askCowallet => _p('问 cowallet', 'Ask cowallet');
  static String get composerHint => _p('跟 cowallet 说点什么…', 'Tell cowallet what you need…');
  static String get intentHeader => _p('我听到的是', "What I'm hearing");
  static String get intentConfirming => _p('让我确认一下…', "Let me make sure I got this…");
  static String get intentMisread => _p('好,我刚理解错了。能再说一次吗?', "Got it, I misread. Say it again?");
  static String get intentOnIt => _p('好,这就办。', 'On it.');
  static String get intentExecuting => _p('正在执行…', 'Working on it…');
  static String get noWallet => _p('还没有创建钱包。', 'No wallet created yet.');
  static String get balanceQueryFailed => _p('查询余额失败', 'Failed to fetch balance');

  // Send view
  static String get sendTitle => _p('转账', 'Send');
  static String get recipient => _p('收款方', 'Recipient');
  static String get addressHint => _p('地址或 ENS 名称', 'Address or ENS name');
  static String get amount => _p('金额', 'Amount');
  static String get enterAddress => _p('请输入收款地址', 'Please enter a recipient address');
  static String get enterValidAmount => _p('请输入有效金额', 'Please enter a valid amount');
  static String get confirmTransfer => _p('确认转账', 'Confirm transfer');
  static String get amountLabel => _p('金额', 'Amount');
  static String get recipientLabel => _p('收款方', 'Recipient');
  static String get estGas => _p('预估 Gas', 'Est. gas');
  static String get sigMethod => _p('签名方式', 'Signing method');
  static String get network => _p('网络', 'Network');
  static String get balancePrefix => _p('余额', 'Balance');
  static String get txSuccess => _p('转账成功', 'Transfer successful');
  static String get txHashLabel => _p('交易哈希', 'Transaction hash');
  static String get done => _p('完成', 'Done');
  static String get txFailed => _p('转账失败', 'Transfer failed');
  static String get gasEstimating => _p('估算中…', 'Estimating…');
  static String get gasEstimateFailed => _p('Gas 估算失败', 'Gas estimation failed');
  static String get invalidAddress => _p('无效的收款地址', 'Invalid recipient address');
  static String get invalidAmount => _p('无效的金额', 'Invalid amount');
  static String get bioAuthFailed => _p('生物认证失败,转账已取消', 'Biometric auth failed, transfer cancelled');

  // Receive view
  static String get receiveTitle => _p('收款', 'Receive');
  static String get addressCopied => _p('地址已复制', 'Address copied');
  static String get copyAddress => _p('复制地址', 'Copy address');
  static String get share => _p('分享', 'Share');
  static String get createWalletFirst => _p('请先创建钱包', 'Create wallet first');

  // Tx history
  static String get txPending => _p('确认中…', 'Confirming…');
  static String get txConfirmed => _p('已确认', 'Confirmed');
  static String get txFailedStatus => _p('失败', 'Failed');
  static String get noTxYet => _p('暂无交易记录', 'No transactions yet');
  static String get demoData => _p('示例数据', 'Demo data');
  static String get copy => _p('复制', 'Copy');
  static String get copied => _p('✓ 已复制', '✓ Copied');
  static String get regenerate => _p('再试一次', 'Regenerate');
  static String get cancel => _p('取消', 'Cancel');

  // Agents
  static String get agentsLabel => _p('助手中心', 'Agents');
  static String get agentsH1 => _p('让 AI 在你定的规矩里帮你办事。', 'Let AI handle things — within rules you set.');
  static String get agentsSub => _p('你写规矩(花多少、给谁、干啥),AI 照办。越界就停。', 'You write the rules (how much, to whom, for what). AI obeys. Crosses a line, it stops.');
  static String get connected => _p('已接入 · 2 个助手', 'Connected · 2 agents');
  static String get freezeAll => _p('紧急全冻', 'Freeze all');
  static String get active => _p('活跃', 'active');
  static String get connectNew => _p('接入新助手', 'Connect a new agent');
  static String get skillsLabel => _p('Skills · 给钱包扩展能力', 'Skills · extensions for your wallet');
  static String get skillsIntro => _p('别人写好的能力,安装就能用。像给浏览器装插件。', 'Prebuilt capabilities. Install and use. Like browser extensions.');
  static String get installed => _p('已用', 'Installed');
  static String get addSkill => _p('+ 装', '+ add');
  static String get devProtocols => _p('开发者接口', 'Developer protocols');

  // Settings
  static String get settings => _p('设置', 'Settings');
  static String get security => _p('安全', 'Security');
  static String get keysCheckup => _p('三份钥匙体检', 'Three-piece key checkup');
  static String get keysCheckupSub => _p('动你的钱,要三份钥匙都点头', 'All three must agree to move your money');
  static String get allSafe => _p('都在', 'safe');
  static String get onPhone => _p('手机里', 'On phone');
  static String get inCloud => _p('云端', 'In cloud');
  static String get recovery => _p('找回码', 'Recovery');
  static String get emergencyFreeze => _p('紧急冻结', 'Emergency freeze');
  static String get emergencyFreezeSub => _p('一键停止所有助手、暂停所有交易', 'Stops all agents, pauses all transactions');
  static String get emergencyContact => _p('紧急联系人', 'Emergency contact');
  static String get emergencyContactSub => _p('丢手机时帮你冻结和恢复', 'Helps freeze and recover if you lose your phone');
  static String get riskGuard => _p('风险拦截', 'Risk guard');
  static String get riskGuardSub => _p('AI 实时盯防钓鱼、授权滥用', 'AI watches for phishing, dodgy approvals');
  static String get conversation => _p('对话', 'Conversation');
  static String get intentMode => _p('意图提示出现时机', 'When intent card appears');
  static String get intentModeSub => _p('回车后弹卡(推荐) 或 打字时浮现', 'Pop-after-enter (default) or float-while-typing');
  static String get onEnter => _p('回车后弹', 'On enter');
  static String get voiceInput => _p('语音输入', 'Voice input');
  static String get voiceInputSub => _p('按住说话,松手发送', 'Hold to talk, release to send');
  static String get on => _p('开', 'on');
  static String get aiModel => _p('AI 模型', 'AI model');
  static String get aiModelSub => _p('Claude Opus 4 · 本地小模型做意图识别', 'Claude Opus 4 · local for intent');
  static String get general => _p('一般', 'General');
  static String get language => _p('语言', 'Language');
  static String get weeklyReport => _p('每周透明度报告', 'Weekly transparency report');
  static String get weeklyReportSub => _p('cowallet 自我体检 · 公开', 'cowallet self-audits · public');
  static String get redoOnboarding => _p('重置引导流程', 'Redo onboarding');
  static String get redoOnboardingSub => _p('从头看一遍', 'See it from the start');
  static String get signoff1 => _p('cowallet · 2026', 'cowallet · 2026');
  static String get signoff2 => _p('会听懂人话的钱包', 'A wallet that listens');

  // Keys
  static String get keysH1a => _p('三份钥匙', 'Three pieces,');
  static String get keysH1b => _p('都点头才能', 'all must agree to');
  static String get keysH1em => _p('动钱', 'move');
  static String get keysSub => _p('没人能单独动你的钱 —— 连 cowallet 公司都进不来。', "Nobody can move your money alone — not even cowallet.");
  static String get keyPhone => _p('手机里那份', 'On this phone');
  static String get keyPhoneWhere => _p('在你手机的安全区里', "In your phone's secure enclave");
  static String get keyPhoneMeta => _p('✓ 完整 · 12 分钟前刚用过', '✓ Intact · used 12 min ago');
  static String get keyCloud => _p('云端那份', 'In the cloud');
  static String get keyCloudWhere => _p('Anchorage 托管 · 美国合规机构', 'Anchorage · US-regulated custodian');
  static String get keyCloudMeta => _p('✓ 心跳 2 分钟前 · 加密完整', '✓ Heartbeat 2 min ago · encrypted');
  static String get keyRecovery => _p('找回码那份', 'Recovery piece');
  static String get keyRecoveryWhere => _p('在你那儿(或没设)', 'In your hands (or unset)');
  static String get keyRecoveryTag => _p('该测了', 'test it');
  static String get keyRecoveryMeta => _p('⚠ 90 天没确认过它还在', '⚠ Not verified in 90 days');
  static String get keyRecoveryAction => _p('现在花 30 秒测一下', 'Test it now (30 seconds)');
  static String get keysExplainLabel => _p('这是怎么回事?', 'How does this work?');
  static String get keysExplainBody => _p(
    '想象家门有三把锁 —— 你身上一把,信任的帮手(云端)一把,保险柜里一把(找回码)。开门必须凑齐两把以上。就算 cowallet 公司被黑,他们也只拿到一把,开不了你的门。',
    "Imagine three locks on your door. You hold one. A trusted helper (cloud) holds one. A safe holds one (recovery). Opening needs at least two. Even if cowallet gets hacked, attackers have only one — your door stays shut.",
  );
  static String get keysTechLabel => _p('技术细节 · 给懂行的', 'Tech detail · for pros');
  static String get keysTechBody => _p(
    '门限签名 (TSS) · 设备端用 Secure Enclave,云端由 Anchorage Digital 托管(SOC2/ISO27001)。任一分片不产生完整私钥。',
    "Threshold signatures (TSS). Device share in Secure Enclave; cloud share at Anchorage Digital (SOC2/ISO27001). No share reconstructs the full private key.",
  );
}
