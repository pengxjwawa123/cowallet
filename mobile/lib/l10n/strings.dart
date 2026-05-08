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
  static String get heroFeat1h => _p('动钱需要两把钥匙', 'Two keys to move money');
  static String get heroFeat1s => _p('三份钥匙分开存，任何单方都动不了', 'Three keys stored apart — no single party can act alone');
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
  static String get pickCreateDesc => _p('最简单。钥匙分成三份存在不同地方，动钱需要两份。不用背一长串词。', 'Easiest. Key split into 3 pieces stored separately — 2 needed to move money. No phrases to memorize.');
  static String get pickCreateTag => _p('推荐', 'Best');
  static String get pickImportTitle => _p('我已经有一个钱包', 'I have a wallet already');
  static String get pickImportDesc => _p('用那 12 或 24 个单词的"找回密码"把它搬进来。', 'Bring it in using those 12 or 24 recovery words.');
  static String get pickHwTitle => _p('我有一个硬件钱包', 'I have a hardware wallet');
  static String get pickHwDesc => _p('Ledger、Trezor 之类的实物钥匙。', 'Ledger, Trezor — physical key devices.');

  // Onboarding — Creating
  static String get creatingH1 => _p('正在帮你把钥匙分成三份', 'Splitting your key into three pieces');
  static String get creatingSub => _p('动你的钱需要任意两份钥匙。三份分开存放，丢了一份还能恢复。完整的钥匙从不出现在任何地方。', 'Moving your money requires any 2 of 3 keys. Stored separately — lose one, the other two still work. The full key never exists in one place.');
  static String get cl1 => _p('第 1 份：存在这台手机里', '1st key: stored on this phone');
  static String get cl2 => _p('第 2 份：存在服务器保险柜', '2nd key: stored in server vault');
  static String get cl3 => _p('第 3 份：由你自己保管', '3rd key: kept by you');
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
  static String get bioH1 => _p('开启生物识别', 'Enable biometric authentication');
  static String get bioSub => _p('就像手机解锁一样，用指纹或面容保护你的钱包。生物信息不会离开这台手机。', 'Just like unlocking your phone. Protect your wallet with fingerprint or face. Biometric data never leaves this device.');
  static String get bioActivate => _p('开启生物识别', 'Turn on biometrics');
  static String get bioSkip => _p('改用密码', 'Use a passcode instead');
  static String get bioDone => _p('生物识别已开启', 'Biometrics ready');

  // Onboarding — PIN
  static String get pinH1 => _p('设置钱包密码', 'Set wallet passcode');
  static String get pinSub => _p('6 位数字密码，每次交易时需要输入。', '6-digit passcode, required for every transaction.');
  static String get pinConfirmH1 => _p('再输入一次', 'Confirm passcode');
  static String get pinConfirmSub => _p('请再输入一遍以确认。', 'Enter the same passcode again to confirm.');
  static String get pinMismatch => _p('两次输入不一致，请重新设置', 'Passcodes don\'t match. Try again.');
  static String get pinDone => _p('密码已设置', 'Passcode set');

  // Onboarding — Name
  static String get nameH1 => _p('我该怎么叫你?', 'What should I call you?');
  static String get nameSub => _p('起个名字就行,不用真名。', 'A nickname works. No real name needed.');
  static String get namePlaceholder => _p('比如 小明 / 老王 / Alice', 'e.g. Alice, Mike, or a nickname');
  static String get nameHint => _p('存在你手机里,不会上传。', 'Stays on your phone. Never uploaded.');
  static String get continueBtn => _p('下一步', 'Continue');

  // Onboarding — Backup (3rd shard)
  static String get backupH1 => _p('保管你的第 3 份钥匙', 'Store your 3rd key');
  static String get backupSub => _p('这是属于你的那份钥匙。动你的钱需要任意两份——手机丢了，用这份 + 服务器就能恢复。', 'This is your personal key. Moving money needs any 2 of 3 — if you lose your phone, this one + server recovers everything.');
  static String get backupSkip => _p('稍后备份 (不推荐)', 'Backup later (not recommended)');
  static String get backupCloudTitle => _p('iCloud / Google Cloud', 'iCloud / Google Cloud');
  static String get backupCloudDesc => _p('加密存储，跨设备自动同步', 'Encrypted, auto-synced across devices');
  static String get backupFileTitle => _p('导出到文件', 'Export to file');
  static String get backupFileDesc => _p('手动保存加密备份文件', 'Save encrypted backup file manually');
  static String get backupSaved => _p('备份完成', 'Backup saved');
  static String get backupSaving => _p('正在保存...', 'Saving...');
  static String get backupErrCloudUnavailable => _p('云备份在此设备上不可用', 'Cloud backup is not available on this device');
  static String get backupErrCloudStoreFailed => _p('云备份保存失败，请重试', 'Cloud backup failed, please try again');
  static String get backupErrFileWriteFailed => _p('文件保存失败，请检查存储权限', 'File save failed, please check storage permissions');
  static String get backupErrShardNotAvailable => _p('备份数据不可用，请重新创建钱包', 'Backup data not available, please recreate wallet');
  static String backupFileSaved(String path) => _p('已保存到: $path', 'Saved to: $path');

  // Onboarding — Ready
  static String get readyH1 => _p('都搞定了。', 'All set.');
  static String readyH1Named(String name) => _p('都搞定了,$name。', "You're in, $name.");
  static String get readySub => _p('你的钱包已经可以用了。', 'Your wallet is ready to use.');
  static String get readyWhat => _p('接下来可以做这些', 'What you can do next');
  static String get ready1h => _p('收到的第一笔钱,gas 我们付', 'First incoming? Gas is on us');
  static String get ready1s => _p('给你试试手,不花你一分钱', 'Try it out, nothing from your pocket');
  static String get ready2h => _p('试着对它说话', 'Try talking to it');
  static String get ready2s => _p('"我的钱都放哪了" "最近有什么花销"', '"Show me my money" "How much did I spend this month?"');
  static String get ready3h => _p('定期检查第 3 份钥匙还在', 'Verify your 3rd key periodically');
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
  static String get homeStatus => _p('一切正常 · 三份钥匙各就各位 · 今天还没可疑动作', 'All good · all three keys in place · no odd activity today');
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
  static String get keysCheckup => _p('三份钥匙体检', 'Three-key checkup');
  static String get keysCheckupSub => _p('动你的钱需要任意两份钥匙', 'Any 2 of 3 keys needed to move your money');
  static String get allSafe => _p('都在', 'safe');
  static String get onPhone => _p('手机里', 'On phone');
  static String get inCloud => _p('云端', 'In cloud');
  static String get recovery => _p('第 3 份钥匙', '3rd Key');
  static String get emergencyFreeze => _p('紧急冻结', 'Emergency freeze');
  static String get emergencyFreezeSub => _p('一键停止所有助手、暂停所有交易', 'Stops all agents, pauses all transactions');
  static String get emergencyContact => _p('紧急联系人', 'Emergency contact');
  static String get emergencyContactSub => _p('丢手机时帮你冻结和恢复', 'Helps freeze and recover if you lose your phone');
  static String get riskGuard => _p('风险拦截', 'Risk guard');
  static String get riskGuardSub => _p('AI 实时盯防钓鱼、授权滥用', 'AI watches for phishing, dodgy approvals');
  static String get biometricAuth => _p('生物认证', 'Biometric authentication');
  static String get biometricAuthSub => _p('用指纹或面容验证敏感操作', 'Use fingerprint or face for sensitive actions');
  static String get biometricAuthReason => _p('验证以继续', 'Authenticate to continue');
  static String get biometricNotAvailable => _p('此设备不支持生物认证', 'Biometric auth not available on this device');
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
  static String get keysH1a => _p('三份钥匙', 'Three keys,');
  static String get keysH1b => _p('两份即可', 'any two to');
  static String get keysH1em => _p('动钱', 'move money');
  static String get keysSub => _p('没人能单独动你的钱 —— 连 cowallet 公司都进不来。丢了一份，剩下两份照样恢复。', "Nobody can move your money alone — not even cowallet. Lose one key, the other two still recover everything.");
  static String get keyPhone => _p('第 1 份 · 手机', '1st key · Phone');
  static String get keyPhoneWhere => _p('存在手机安全芯片里 (Secure Enclave / StrongBox)', "In your phone's secure chip (Secure Enclave / StrongBox)");
  static String get keyPhoneMeta => _p('✓ 完整 · 12 分钟前刚用过', '✓ Intact · used 12 min ago');
  static String get keyCloud => _p('第 2 份 · 服务器', '2nd key · Server');
  static String get keyCloudWhere => _p('cowallet 服务器 · HSM 保护', 'cowallet server · HSM protected');
  static String get keyCloudMeta => _p('✓ 心跳 2 分钟前 · 加密完整', '✓ Heartbeat 2 min ago · encrypted');
  static String get keyRecovery => _p('第 3 份 · 你保管', '3rd key · Yours');
  static String get keyRecoveryWhere => _p('由你存在 iCloud / Google Drive', 'Stored by you in iCloud / Google Drive');
  static String get keyRecoveryWhereFile => _p('由你保存在本地 JSON 文件', 'Stored by you as a local JSON file');
  static String get keyRecoveryTag => _p('该测了', 'test it');
  static String get keyRecoveryMeta => _p('⚠ 90 天没确认过它还在', '⚠ Not verified in 90 days');
  static String get keyRecoveryAction => _p('现在花 30 秒测一下', 'Test it now (30 seconds)');
  static String get keyRecoveryActionFile => _p('导入本地文件验证', 'Import local file to verify');

  // Key health status
  static String get justNow => _p('刚刚', 'just now');
  static String minutesAgo(int n) => _p('$n 分钟前', '$n min ago');
  static String hoursAgo(int n) => _p('$n 小时前', '$n hours ago');
  static String daysAgo(int n) => _p('$n 天前', '$n days ago');
  static String get keyIntact => _p('完整', 'Intact');
  static String keyLastUsed(String time) => _p('上次使用 $time', 'last used $time');
  static String keyHeartbeat(String time) => _p('心跳 $time · 加密完整', 'heartbeat $time · encrypted');
  static String keyVerified(String time) => _p('已验证 $time', 'verified $time');
  static String get keyNotVerified => _p('尚未验证', 'Not yet verified');
  static String keyNotVerifiedDays(int n) => _p('$n 天没确认过它还在', 'Not verified in $n days');
  static String get keyUnavailable => _p('不可用', 'Unavailable');
  static String get keyServerUnreachable => _p('服务器无法连接', 'Server unreachable');
  static String get keyServerWarning => _p('服务器响应异常', 'Server response abnormal');
  static String get backupTestSuccess => _p('第 3 份钥匙验证通过', '3rd key verified successfully');
  static String get backupTestFailed => _p('第 3 份钥匙验证失败，请检查备份', '3rd key verification failed, please check your backup');

  static String get keysExplainLabel => _p('这是怎么回事?', 'How does this work?');
  static String get keysExplainBody => _p(
    '想象家门有三把锁 —— 手机里一把、服务器一把、你自己保管一把。开门只需要任意两把。就算 cowallet 服务器被黑，攻击者也只拿到一把，开不了你的门。丢了手机？用你保管的 + 服务器就能恢复。',
    "Imagine three locks on your door — one on your phone, one on the server, one kept by you. Opening only needs any two. Even if cowallet's server gets hacked, attackers only have one — your door stays shut. Lost your phone? Your backup + server recovers everything.",
  );
  static String get keysTechLabel => _p('技术细节 · 给懂行的', 'Tech detail · for pros');
  static String get keysTechBody => _p(
    '2-of-3 门限签名 (DKLS23 TSS) · 设备端用 Secure Enclave / StrongBox，服务端 HSM 保护。任意两方可签名，单一分片无法还原完整私钥。',
    "2-of-3 threshold signatures (DKLS23 TSS). Device share in Secure Enclave / StrongBox; server share protected by HSM. Any two parties can sign; no single share reconstructs the full private key.",
  );

  // Key Security (Reshare)
  static String get keySecurity => _p('密钥安全', 'Key Security');
  static String get rotateKeyShares => _p('轮转三份钥匙', 'Rotate Keys');
  static String get rotateKeySharesSub => _p('刷新三份钥匙，旧的失效，钱包地址不变', 'Refresh all three keys, old ones invalidated, wallet address unchanged');
  static String get lastRotation => _p('上次轮转', 'Last rotation');
  static String get never => _p('从未', 'Never');
  static String get autoRotate => _p('每 30 天自动轮转', 'Auto-rotate every 30 days');
  static String get autoRotateSub => _p('定期刷新三份钥匙以提高安全性', 'Periodically refresh all three keys for better security');
  static String get rotating => _p('轮转中...', 'Rotating...');
  static String get rotationSuccess => _p('三份钥匙已刷新', 'All three keys refreshed');
  static String get rotationFailed => _p('钥匙轮转失败', 'Key rotation failed');

  // Presignatures
  static String get presignatures => _p('预签名', 'Presignatures');
  static String get presignaturesAvailable => _p('可用预签名', 'Available presignatures');
  static String get generatePresignatures => _p('生成预签名', 'Generate Presignatures');
  static String get presignaturesSub => _p('预计算签名材料，加速交易执行', 'Pre-computed signing material for faster transactions');
  static String get selectCount => _p('选择数量', 'Select count');
  static String get generating => _p('生成中...', 'Generating...');
  static String get generationSuccess => _p('预签名生成成功', 'Presignatures generated successfully');
  static String get generationFailed => _p('预签名生成失败', 'Presignature generation failed');
  static String get generate => _p('生成', 'Generate');
}
