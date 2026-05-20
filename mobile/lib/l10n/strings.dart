enum Lang { zh, en }

class S {
  S._();

  static Lang _lang = Lang.zh;
  static Lang get lang => _lang;
  static void setLang(Lang l) => _lang = l;

  static String _p(String zh, String en) => _lang == Lang.zh ? zh : en;

  // App
  static String get appName => 'CoWallet';
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
  static String get heroFeat1h => _p('不用懂区块链', 'No crypto knowledge needed');
  static String get heroFeat1s => _p('说句话就能转账、收款、理财', 'Send, receive, and earn just by saying so');
  static String get heroFeat2h => _p('100+ 个金融网络', '100+ financial networks');
  static String get heroFeat2s => _p('全世界通用', 'Works worldwide');
  static String get heroFeat3h => _p('AI 帮你跑腿', 'AI does the errands');
  static String get heroFeat3s => _p('你只需说一句话', 'Just say the word');
  static String get getStarted => _p('开始使用', 'Get started');
  static String get heroLegal => _p('继续即表示同意服务条款与隐私政策', 'By continuing you agree to our Terms and Privacy Policy');

  // Onboarding — Intro (MPC explanation)
  static String get introH1 => _p('你的钱包如何保护你', 'How your wallet protects you');
  static String get introSub => _p('CoWallet 用一种叫"门限签名"的技术，把钥匙拆成三份。', 'CoWallet uses threshold signatures to split your key into three pieces.');
  static String get introBullet1h => _p('钥匙拆成三份', 'Key split into three');
  static String get introBullet1s => _p('手机一份、服务器一份、你自己保管一份。完整钥匙从不出现在任何地方。', 'One on your phone, one on server, one kept by you. The full key never exists anywhere.');
  static String get introBullet2h => _p('动钱需要两份', 'Two needed to transact');
  static String get introBullet2s => _p('任何单方（包括 CoWallet）都无法单独动你的钱。', 'No single party — including CoWallet — can move your money alone.');
  static String get introBullet3h => _p('没有助记词', 'No seed phrase');
  static String get introBullet3s => _p('不用抄 12 个单词。丢了手机，用你的备份 + 服务器就能恢复。', 'No 12 words to write down. Lose your phone, your backup + server recovers everything.');
  static String get introStart => _p('开始创建', 'Start creating');

  // Onboarding — Email
  static String get emailH1 => _p('绑定恢复邮箱', 'Recovery Email');
  static String get emailSub => _p('用于账户恢复时验证身份，我们不会发送垃圾邮件。', 'Used to verify your identity during wallet recovery. We won\'t send spam.');
  static String get emailHint => _p('此邮箱仅用于钱包恢复验证', 'This email is only used for wallet recovery verification');
  static String get invalidEmail => _p('请输入有效的邮箱地址', 'Please enter a valid email address');
  static String get emailSendFailed => _p('发送验证码失败，请重试', 'Failed to send code, please try again');
  static String get emailAlreadyRegistered => _p('该邮箱已注册', 'Email already registered');
  static String get emailAlreadyRegisteredDesc => _p('该邮箱已关联钱包，是否前往恢复流程？', 'This email is linked to an existing wallet. Go to recovery?');
  static String get goRecovery => _p('去恢复', 'Recover');
  static String get reRegister => _p('重新注册', 'Re-register');
  static String get reRegisterDesc => _p('将创建新钱包，原钱包资产需通过恢复流程找回', 'This will create a new wallet. Original assets can only be recovered via the recovery flow.');
  static String get reRegisterConfirm => _p('确认重新注册', 'Confirm Re-register');

  // Onboarding — Email OTP
  static String get otpH1 => _p('输入验证码', 'Enter Verification Code');
  static String otpSub(String email) => _p('验证码已发送至 $email', 'Code sent to $email');
  static String get otpResend => _p('重新发送验证码', 'Resend code');
  static String get otpInvalid => _p('验证码错误或已过期', 'Invalid or expired code');

  // Onboarding — Creating
  static String get creatingH1 => _p('正在帮你把钥匙分成三份', 'Splitting your key into three pieces');
  static String get creatingSub => _p('动你的钱需要任意两份钥匙。三份分开存放，丢了一份还能恢复。完整的钥匙从不出现在任何地方。', 'Moving your money requires any 2 of 3 keys. Stored separately — lose one, the other two still work. The full key never exists in one place.');
  static String get cl1 => _p('第 1 份：存在这台手机里', '1st key: stored on this phone');
  static String get cl2 => _p('第 2 份：存在服务器保险柜', '2nd key: stored in server vault');
  static String get cl3 => _p('第 3 份：由你自己保管', '3rd key: kept by you');
  static String get createError => _p('钱包创建失败,请重试。', 'Wallet creation failed. Please try again.');
  static String get retry => _p('重试', 'Retry');


  // Onboarding — Bio
  static String get bioH1 => _p('开启生物识别', 'Enable biometric authentication');
  static String get bioSub => _p('就像手机解锁一样，用指纹或面容保护你的钱包。生物信息不会离开这台手机。', 'Just like unlocking your phone. Protect your wallet with fingerprint or face. Biometric data never leaves this device.');
  static String get bioActivate => _p('开启生物识别', 'Turn on biometrics');
  static String get bioSkip => _p('改用密码', 'Use a passcode instead');
  static String get bioVerifying => _p('正在验证...', 'Verifying...');
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
  static String get onboardingIncompleteBanner => _p('⚠ 钱包创建流程未完成', '⚠ Wallet setup incomplete');
  static String get onboardingIncompleteAction => _p('继续完成', 'Continue setup');
  static String get onboardingIncompleteUrgent => _p('钱包创建流程已超过 7 天未完成，请尽快完成', 'Wallet setup incomplete for over 7 days, please finish soon');

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
  static String get homeSlogan => _p('说句话就能转账查账，不用点来点去', 'Just say it — no buttons needed');
  static String get yourTotal => _p('你的总资产', 'Your total');
  static String get send => _p('发送', 'Send');
  static String get receive => _p('收款', 'Receive');
  static String get scan => _p('扫码', 'Scan');
  static String get people => _p('联系人', 'People');
  static String get tryTalking => _p('试试跟它说话', 'Try talking to it');
  static String get actionSend => _p('转点币', 'Send some crypto');
  static String get actionReceive => _p('收款地址', 'My address');
  static String get actionSwap => _p('换个币', 'Swap tokens');
  static String actionTokenInfo(String symbol, String balance, String usd) =>
      _p('看看 $symbol，余额 $balance，约 \$$usd',
          'Check $symbol, balance $balance, ~\$$usd');
  static String actionTokenInfoOnChain(String symbol, String balance, String usd, String chain) =>
      _p('看看 $chain 上的 $symbol，余额 $balance，约 \$$usd',
          'Check $symbol on $chain, balance $balance, ~\$$usd');
  static String get actionScan => _p('扫码付款', 'Scan to pay');
  static String get actionPeople => _p('联系人', 'Contacts');
  static String get recentActivity => _p('最近发生的事', 'Recent activity');
  static String get seeAll => _p('全部', 'See all');
  static String get onlyCowallet => _p('只有 CoWallet 有的', 'Only CoWallet does this');
  static String get today => _p('今天', 'today');

  // Home — Try prompts
  static String get try1h => _p('帮我看看还有多少币', 'What do I have?');
  static String get try1s => _p('全链资产一目了然', 'All chains at a glance');
  static String get try2h => _p('转 0.1 ETH 给朋友', 'Send 0.1 ETH to a friend');
  static String get try2s => _p('智能选链、Gas 预估', 'Smart chain pick & gas estimate');
  static String get try3h => _p('最近花了些啥', 'What did I spend recently?');
  static String get try3s => _p('多链交易记录汇总', 'Cross-chain tx summary');

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
  static String get newChat => _p('新对话', 'New chat');
  static String get chatHistory => _p('历史记录', 'History');
  static String get noSessions => _p('暂无历史对话', 'No past conversations');
  static String get deleteSession => _p('删除对话', 'Delete conversation');
  static String get deleteSessionConfirm => _p('确定删除这条对话?', 'Delete this conversation?');
  static String get confirm => _p('确定', 'Confirm');
  static String get chatEmpty => _p('说点什么?', "What's on your mind?");
  static String get chatEmptySub => _p('说话、打字、拍张照都行。', 'Talk, type, or send a photo.');
  static String get askCowallet => _p('问 CoWallet', 'Ask CoWallet');
  static String get composerHint => _p('想干嘛，说一句就行…', 'Just say what you need…');
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
  static String get txStatus => _p('交易状态', 'Transaction Status');
  static String get txSuccess => _p('转账成功', 'Transfer successful');
  static String get txHashLabel => _p('交易哈希', 'Transaction hash');
  static String get done => _p('完成', 'Done');
  static String get txFailed => _p('交易失败', 'Transaction failed');
  static String get blockNumber => _p('区块号', 'Block');
  static String get gasUsed => _p('Gas 消耗', 'Gas used');
  static String get gasEstimating => _p('估算中…', 'Estimating…');
  static String get gasEstimateFailed => _p('Gas 估算失败', 'Gas estimation failed');
  static String get invalidAddress => _p('无效的收款地址', 'Invalid recipient address');
  static String get invalidAmount => _p('无效的金额', 'Invalid amount');
  static String get bioAuthFailed => _p('身份验证失败，转账已取消', 'Authentication failed, transfer cancelled');
  static String get authFailed => _p('身份验证失败', 'Authentication failed');

  // Policy engine
  static String get policyChecking => _p('正在检查交易策略…', 'Checking transaction policy…');
  static String get policyDeniedTitle => _p('交易被拒绝', 'Transaction Denied');
  static String get policyDeniedDefault => _p('此交易不符合当前安全策略', 'This transaction violates your security policy');
  static String get policyApprovalTitle => _p('需要额外确认', 'Additional Approval Required');
  static String get policyApprovalDefault => _p('此交易需要联合签名人批准', 'This transaction requires co-signer approval');
  static String get policyApprovalProceed => _p('请求批准', 'Request Approval');
  static String get policyDailyLimitExceeded => _p('超出每日转账限额', 'Exceeds your daily transfer limit');
  static String get policySingleTxLimitExceeded => _p('单笔金额超过上限', 'Single transaction amount exceeds limit');
  static String get policyNewRecipientWarning => _p('首次向此地址转账，请确认', 'First transfer to this address, please confirm');
  static String get policyHighRiskContract => _p('检测到高风险合约交互', 'High-risk contract interaction detected');
  static String get policyOk => _p('我知道了', 'OK');

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
  static String get keyStatusError => _p('异常', 'error');
  static String get keyStatusWarning => _p('待验证', 'verify');
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
  static String get weeklyReportSub => _p('CoWallet 自我体检 · 公开', 'CoWallet self-audits · public');
  static String get redoOnboarding => _p('重置引导流程', 'Redo onboarding');
  static String get redoOnboardingSub => _p('从头看一遍', 'See it from the start');
  static String get off => _p('关', 'off');
  static String get whileTyping => _p('打字时浮现', 'While typing');
  static String get emergencyFreezeConfirmTitle => _p('确认紧急冻结', 'Confirm Emergency Freeze');
  static String get emergencyFreezeConfirmBody => _p(
    '确定要冻结吗？这会暂停所有交易和助手执行。',
    'Are you sure? This will pause all transactions.',
  );
  static String get emergencyFreezeActivated => _p('紧急冻结已激活', 'Emergency freeze activated');
  static String get emergencyFreezeDeactivated => _p('紧急冻结已解除', 'Emergency freeze deactivated');
  static String get frozenBanner => _p('⚠ 所有交易和助手已冻结', '⚠ All transactions and agents frozen');
  static String get signoff1 => _p('CoWallet · 2026', 'CoWallet · 2026');
  static String get signoff2 => _p('会听懂人话的钱包', 'A wallet that listens');

  // Keys
  static String get keysH1a => _p('三份钥匙', 'Three keys,');
  static String get keysH1b => _p('两份即可', 'any two to');
  static String get keysH1em => _p('动钱', 'move money');
  static String get keysSub => _p('没人能单独动你的钱 —— 连 CoWallet 公司都进不来。丢了一份，剩下两份照样恢复。', "Nobody can move your money alone — not even CoWallet. Lose one key, the other two still recover everything.");
  static String get keyPhone => _p('第 1 份 · 手机', '1st key · Phone');
  static String get keyPhoneWhere => _p('存在手机安全芯片里 (Secure Enclave / StrongBox)', "In your phone's secure chip (Secure Enclave / StrongBox)");
  static String get keyPhoneMeta => _p('✓ 完整 · 12 分钟前刚用过', '✓ Intact · used 12 min ago');
  static String get keyCloud => _p('第 2 份 · 服务器', '2nd key · Server');
  static String get keyCloudWhere => _p('CoWallet 服务器 · HSM 保护', 'CoWallet server · HSM protected');
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
    '想象家门有三把锁 —— 手机里一把、服务器一把、你自己保管一把。开门只需要任意两把。就算 CoWallet 服务器被黑，攻击者也只拿到一把，开不了你的门。丢了手机？用你保管的 + 服务器就能恢复。',
    "Imagine three locks on your door — one on your phone, one on the server, one kept by you. Opening only needs any two. Even if CoWallet's server gets hacked, attackers only have one — your door stays shut. Lost your phone? Your backup + server recovers everything.",
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

  // Scan
  static String get scanTitle => _p('扫一扫', 'Scan QR');
  static String get scanPermissionDenied => _p('需要相机权限才能扫码', 'Camera permission is required to scan');
  static String get scanOpenSettings => _p('去设置', 'Open Settings');
  static String get scanFlashOn => _p('关闭闪光灯', 'Flash off');
  static String get scanFlashOff => _p('打开闪光灯', 'Flash on');
  static String get scanHint => _p('将二维码放入框内', 'Align QR code within frame');
  static String scanTransferTo(String address) => _p('转账给 $address', 'Transfer to $address');
  static String scanTransferAmount(String amount, String token, String address) =>
      _p('转 $amount $token 给 $address', 'Send $amount $token to $address');

  // Chain selector
  static String get selectNetwork => _p('选择网络', 'Select network');
  static String get mainnets => _p('主网', 'MAINNETS');
  static String get testnets => _p('测试网', 'TESTNETS');
  static String get testnetBadge => _p('测试', 'TEST');

  // Contacts
  static String get contactsTitle => _p('联系人', 'Contacts');
  static String get contactsAdd => _p('添加联系人', 'Add contact');
  static String get contactsEdit => _p('编辑联系人', 'Edit contact');
  static String get contactsDelete => _p('删除联系人', 'Delete contact');
  static String get contactsDeleteConfirm => _p('确定删除这个联系人?', 'Delete this contact?');
  static String get contactsSearch => _p('搜索姓名或地址', 'Search name or address');
  static String get contactsEmpty => _p('还没有联系人', 'No contacts yet');
  static String get contactsName => _p('姓名', 'Name');
  static String get contactsNameHint => _p('比如 小明 / Alice', 'e.g. Alice, Mike');
  static String get contactsNameRequired => _p('请输入姓名', 'Name is required');
  static String get contactsAddress => _p('钱包地址', 'Wallet address');
  static String get contactsAddressRequired => _p('请输入钱包地址', 'Address is required');
  static String get contactsAddressInvalid => _p('无效的地址 (需要 0x 开头, 42 位)', 'Invalid address (must be 0x, 42 chars)');
  static String get contactsNote => _p('备注', 'Note');
  static String get contactsNoteHint => _p('可选备注', 'Optional note');
  static String get contactsSave => _p('保存', 'Save');

  // Notifications
  static String get notifTxConfirmedTitle => _p('转账已确认', 'Transfer Confirmed');
  static String notifTxConfirmedBody(String amount, String token, String hash) =>
      _p('$amount $token 已成功发送 ($hash)', '$amount $token sent successfully ($hash)');
  static String get notifTxFailedTitle => _p('转账失败', 'Transfer Failed');
  static String notifTxFailedBody(String hash, String reason) =>
      _p('交易 $hash 失败: $reason', 'Transaction $hash failed: $reason');
  static String get notifSecurityAlertTitle => _p('安全警告', 'Security Alert');
  static String get notifChannelTransactions => _p('交易通知', 'Transactions');
  static String get notifChannelSecurity => _p('安全警报', 'Security Alerts');

  // Yield / DeFi
  static String get yieldLabel => _p('DeFi 赚钱', 'DeFi Earn');
  static String get yieldH1 => _p('让闲钱去干活。', 'Put idle money to work.');
  static String get yieldSub => _p('链上理财协议,年化 3%–40%。风险分级一目了然。', 'On-chain yield protocols, 3%–40% APY. Risk levels at a glance.');
  static String get yieldOpportunities => _p('收益机会', 'Opportunities');
  static String get yieldAll => _p('全部', 'All');
  static String get yieldLending => _p('借贷', 'Lending');
  static String get yieldStaking => _p('质押', 'Staking');
  static String get yieldVault => _p('金库', 'Vault');
  static String get yieldFarm => _p('挖矿', 'Farm');
  static String get yieldDeposit => _p('存入', 'Deposit');
  static String get yieldDepositNow => _p('立即存入', 'Deposit now');
  static String get yieldStrategy => _p('策略说明', 'Strategy');
  static String get yieldApyBreakdown => _p('APY 构成', 'APY breakdown');
  static String get yieldBaseApy => _p('基础利率', 'Base APY');
  static String get yieldRewardApy => _p('奖励', 'Reward APY');
  static String get yieldIncentiveApy => _p('激励', 'Incentive APY');
  static String get yieldRisks => _p('风险因素', 'Risk factors');
  static String get yieldRiskLow => _p('低风险', 'Low');
  static String get yieldRiskMed => _p('中风险', 'Medium');
  static String get yieldRiskHigh => _p('高风险', 'High');
  static String get yieldRiskVeryHigh => _p('极高风险', 'Very high');
  static String get yieldEmpty => _p('暂无收益机会', 'No opportunities available');
  static String get yieldLoadFailed => _p('加载失败', 'Failed to load');
  static String get yieldBestApy => _p('最高', 'Best');
  static String get yieldAvgApy => _p('平均', 'Avg');
  static String get tabDefi => _p('赚钱', 'Earn');

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

  // Recovery
  static String get recoveryH1 => _p('恢复你的钱包', 'Recover your wallet');
  static String get recoverySub => _p('丢了手机？用你的备份钥匙 + 服务器就能恢复。输入注册时的邮箱开始。', 'Lost your phone? Use your backup key + server to recover. Enter your email to begin.');
  static String get recoveryEmailHint => _p('注册时的邮箱地址', 'Email used during signup');
  static String get recoveryEmailInvalid => _p('请输入有效的邮箱地址', 'Please enter a valid email address');
  static String get recoverySendOtp => _p('发送验证码', 'Send verification code');
  static String get recoveryCancel => _p('取消', 'Cancel');
  static String get recoveryOtpH1 => _p('输入验证码', 'Enter verification code');
  static String get recoveryOtpSub => _p('我们已经发送了一个验证码到你的邮箱。', 'We sent a verification code to your email.');
  static String get recoveryOtpInvalid => _p('请输入完整的验证码', 'Please enter the full verification code');
  static String get recoveryVerify => _p('验证', 'Verify');
  static String get recoveryBackupH1 => _p('导入你的备份钥匙', 'Import your backup key');
  static String get recoveryBackupSub => _p('选择你当初备份第 3 份钥匙的方式来导入。', 'Import using the method you used to back up your 3rd key.');
  static String get recoveryFromCloud => _p('从 iCloud / Google Cloud', 'From iCloud / Google Cloud');
  static String get recoveryFromCloudDesc => _p('自动检索云端备份', 'Automatically retrieve cloud backup');
  static String get recoveryFromFile => _p('从备份文件', 'From backup file');
  static String get recoveryFromFileDesc => _p('选择之前导出的 JSON 文件', 'Select the JSON file you exported');
  static String get recoveryImporting => _p('正在导入备份...', 'Importing backup...');
  static String get recoveryInProgress => _p('正在恢复钱包', 'Recovering wallet');
  static String get recoveryInProgressSub => _p('正在用你的备份和服务器重新生成手机钥匙。', 'Regenerating your device key using backup + server.');
  static String get recoveryStep1 => _p('身份已验证', 'Identity verified');
  static String get recoveryStep2 => _p('备份钥匙已导入', 'Backup key imported');
  static String get recoveryStep3 => _p('生成新的手机钥匙', 'Generating new device key');
  static String get recoveryDoneH1 => _p('钱包已恢复', 'Wallet recovered');
  static String get recoveryDoneSub => _p('你的钱包已成功恢复到这台设备。所有资产安全无损。', 'Your wallet has been restored to this device. All assets safe and sound.');
  static String get recoveryGoHome => _p('进入钱包', 'Go to wallet');
  static String get recoverWallet => _p('恢复已有钱包', 'Recover existing wallet');

  // Intent executor / Chat
  static String get emergencyFreezeActive => _p('紧急冻结已激活，所有操作已暂停。请先在设置中解除冻结。', 'Emergency freeze is active. All operations paused. Deactivate in Settings first.');
  static String get onIt => _p('好,这就办。', 'On it.');
  static String yourBalance(String eth, String usdc) => _p('你的余额: $eth + $usdc', 'Your balance: $eth + $usdc');
  static String errorMsg(String err) => _p('出错了: $err', 'Error: $err');
  static String get invalidRecipient => _p('无效的收款地址', 'Invalid recipient address');
  static String get insufficientGas => _p('余额不足以支付Gas费', 'Insufficient balance for gas');
  static String sendAllRequiresGasDeduction(String balance, String maxSendable, String symbol, String gasCost) => _p(
    '转出全部余额需扣除Gas费。余额 $balance $symbol，扣除Gas费后实际转出 $maxSendable $symbol，是否继续？',
    'Sending all requires gas deduction. Balance: $balance $symbol, actual send: $maxSendable $symbol after gas. Continue?',
  );
  static String tokenContractNotFound(String token) => _p('未找到代币 $token 的合约地址', 'Contract address for $token not found');
  static String tokenBalanceZero(String token) => _p('$token 余额为零', '$token balance is zero');
  static String insufficientForAmountPlusGas(String maxSendable, String symbol, String gasCost) => _p(
    '余额不足以支付转账金额+Gas费。扣除Gas费后最多可转出 $maxSendable $symbol (Gas≈$gasCost $symbol)，是否继续？',
    'Insufficient balance for amount + gas. Max sendable after gas: $maxSendable $symbol (gas≈$gasCost $symbol). Continue?',
  );
  static String tokenContractNotFoundConfirm(String token) => _p(
    '未找到代币 $token 的合约地址，请确认你持有该代币',
    'Contract address for $token not found. Make sure you hold this token.',
  );
  static String transferSuccess(String shortHash) => _p('转账成功! 交易: $shortHash', 'Transfer sent! Tx: $shortHash');
  static String get authFailedTransferCancelled => _p('身份验证失败，转账已取消', 'Authentication failed, transfer cancelled');
  static String get insufficientBalance => _p('余额不足', 'Insufficient balance');
  static String transferFailed(String msg) => _p('转账失败: $msg', 'Transfer failed: $msg');
  static String get specifySwapTokens => _p('请指定兑换的代币', 'Please specify swap tokens');
  static String insufficientTokenBalance(String token) => _p('$token 余额不足', 'Insufficient $token balance');
  static String swapRouteFailed(String err) => _p('获取兑换路由失败: $err', 'Failed to get swap route: $err');
  static String get invalidSwapData => _p('兑换交易数据无效', 'Invalid swap transaction data');
  static String swapSuccess(String amountFrom, String tokenFrom, String amountTo, String tokenTo, String shortHash) => _p(
    '兑换成功! $amountFrom $tokenFrom → $amountTo $tokenTo\n交易: $shortHash',
    'Swap successful! $amountFrom $tokenFrom → $amountTo $tokenTo\nTx: $shortHash',
  );
  static String get authFailedSwapCancelled => _p('身份验证失败，兑换已取消', 'Authentication failed, swap cancelled');
  static String get tokenApprovalRequired => _p('需要先授权代币额度，请稍后重试', 'Token approval required. Please try again shortly.');
  static String swapFailed(String msg) => _p('兑换失败: $msg', 'Swap failed: $msg');

  // Chat view messages
  static String get requestFailed => _p('请求失败，请稍后重试', 'Request failed, please try again');
  static String get networkError => _p('网络错误，请稍后重试', 'Network error, please try again');
  static String get insufficientGasWarning => _p('⚠ 余额不足以支付Gas费', '⚠ Insufficient balance for gas');
  static String tokenBalanceZeroWarning(String token) => _p('⚠ $token 余额为零', '⚠ $token balance is zero');
  static String get transferCancelled => _p('好的，已取消转账。', 'OK, transfer cancelled.');
  static String get swapCancelled => _p('好的，已取消兑换。', 'OK, swap cancelled.');
  static String get sendAll => _p('全部', 'All');
  static String get thinking => _p('思考中', 'Thinking');

  // Widget labels
  static String get transferSubmitted => _p('转账已提交', 'Transfer submitted');
  static String get amountAdjustedGas => _p('金额已调整（需预留Gas）', 'Amount adjusted (gas reserved)');
  static String get transferConfirm => _p('转账确认', 'Confirm transfer');
  static String get calculatingFees => _p('计算费用中...', 'Calculating fees...');
  static String get amountPlusGasExceeded => _p('转出金额+Gas超出余额，已自动调减', 'Amount + gas exceeds balance, auto-adjusted');
  static String get originalAmount => _p('原始金额', 'Original amount');
  static String get gasFee => _p('Gas 费用', 'Gas fee');
  static String get actualSend => _p('实际转出', 'Actual send');
  static String get recipientAddress => _p('收款地址', 'Recipient address');
  static String get contract => _p('合约', 'Contract');
  static String get estimatedGas => _p('预估 Gas', 'Est. gas');
  static String get estimating => _p('估算中...', 'Estimating...');
  static String get confirmSend => _p('确认转出', 'Confirm send');
  static String get swapSubmitted => _p('兑换已提交', 'Swap submitted');
  static String get swapConfirm => _p('兑换确认', 'Swap confirm');
  static String get pay => _p('支付', 'Pay');
  static String get estimatedReceive => _p('预计获得', 'Est. receive');
  static String get slippageTolerance => _p('滑点容忍', 'Slippage tolerance');
  static String get route => _p('路由', 'Route');
  static String get confirmSwap => _p('确认兑换', 'Confirm swap');

  // Transaction details
  static String get transfer => _p('转账', 'Transfer');
  static String get sender => _p('发送方', 'Sender');
  static String get receiver => _p('接收方', 'Receiver');
  static String get block => _p('区块', 'Block');
  static String get time => _p('时间', 'Time');
  static String get txHashCopied => _p('交易哈希已复制', 'Transaction hash copied');
  static String labelCopied(String label) => _p('$label 已复制', '$label copied');
  static String get confirmed => _p('已确认', 'Confirmed');
  static String get failed => _p('失败', 'Failed');
  static String get pending => _p('待确认', 'Pending');
  static String get unknown => _p('未知', 'Unknown');

  // Transaction history
  static String get txHistory => _p('交易记录', 'Transaction history');
  static String txCount(int count) => _p('共 $count 笔', '$count total');
  static String get noTxHistory => _p('暂无交易记录', 'No transaction history');
  static String moreTxCount(int count) => _p('还有 $count 笔交易...', '$count more transactions...');
  static String sendTo(String addr) => _p('发送至 $addr', 'Send to $addr');

  // Transaction result widget
  static String confirmedWithBlocks(int blocks) => _p('已确认 ($blocks blocks)', 'Confirmed ($blocks blocks)');
  static String get confirming => _p('确认中...', 'Confirming...');

  // Balance widget
  static String get multiChainAssets => _p('多链资产总览', 'Multi-chain assets');
  static String get assetsOverview => _p('资产总览', 'Assets overview');
  static String get noAssetData => _p('暂无资产数据', 'No asset data');

  // Chat suggestions
  static String get suggestBalance => _p('我的余额是多少', "What's my balance");
  static String get suggestRecentTx => _p('最近的交易记录', 'Recent transactions');
  static String get suggestSecurityAudit => _p('安全审计', 'Security audit');
  static String get suggestAddress => _p('我的收款地址', 'Show my address');

  // Backup Shard Export/Import
  static String get backupExport => _p('导出备份钥匙', 'Export backup key');
  static String get backupImport => _p('导入备份钥匙', 'Import backup key');
  static String get backupExportSub => _p('用密码加密你的第 3 份钥匙，生成可扫描的二维码或文件', 'Encrypt your 3rd key with a password, generate a scannable QR code or file');
  static String get backupImportSub => _p('扫描二维码或粘贴加密数据，输入密码解密恢复', 'Scan QR code or paste encrypted data, enter password to decrypt');
  static String get backupPasswordHint => _p('输入备份密码（至少 8 位）', 'Enter backup password (min 8 characters)');
  static String get backupPasswordConfirmHint => _p('再次确认密码', 'Confirm password');
  static String get backupPasswordMismatch => _p('两次密码不一致', 'Passwords do not match');
  static String get backupPasswordTooShort => _p('密码至少 8 位', 'Password must be at least 8 characters');
  static String get backupExporting => _p('正在加密导出...', 'Encrypting and exporting...');
  static String get backupExportSuccess => _p('备份导出成功', 'Backup exported successfully');
  static String get backupExportFailed => _p('备份导出失败', 'Backup export failed');
  static String get backupImporting => _p('正在解密导入...', 'Decrypting and importing...');
  static String get backupImportSuccess => _p('备份导入成功', 'Backup imported successfully');
  static String get backupImportFailed => _p('备份导入失败', 'Backup import failed');
  static String get backupWrongPassword => _p('密码错误或数据已损坏', 'Wrong password or data corrupted');
  static String get backupCopyToClipboard => _p('复制到剪贴板', 'Copy to clipboard');
  static String get backupCopied => _p('已复制到剪贴板', 'Copied to clipboard');
  static String get backupSaveToFile => _p('保存到文件', 'Save to file');
  static String get backupPasteData => _p('粘贴加密数据', 'Paste encrypted data');
  static String get backupScanQr => _p('扫描二维码', 'Scan QR code');
  static String get backupEncryptedData => _p('加密备份数据', 'Encrypted backup data');
  static String get backupNotExported => _p('尚未导出备份', 'Backup not yet exported');
  static String get backupReminder => _p('请尽快导出你的第 3 份钥匙以确保钱包可恢复', 'Please export your 3rd key to ensure wallet recovery');
}
