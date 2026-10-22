# macOS Keychain Wallet

[English](README.md) | **中文**

独立的本机钱包小工具：Swift 负责安全输入、macOS Keychain 和系统认证；Rust 静态库使用成熟实现派生 secp256k1 / SLIP-0010 Ed25519 密钥；Rust 桥接通过 napi-rs 暴露给 Node，负责原生进程通信、缓存和结果复核；TypeScript 只保留类型与加载封装，业务自行使用自己的钱包库。

**当前状态：已完成 Personal Team 签名构建，并用公开无资金测试钱包通过真实 Keychain 保存、系统认证读取、Node 接入和 viem/ethers 离线消息签名验证。** 已实际操作两种算法的图形创建与导入；Ed25519 完整创建、Copy → Paste 确认、错误公钥拒绝、正确导入和后续认证读取均通过。真实资金交易未验证。详见 [验证记录](docs/verification.md)。未读取其他现有钱包秘密，没有后台服务、网络监听、自动任务和遥测。

本版本的 `getKeyPair` 必须显式传 `curve`，支持 `secp256k1` 和 `ed25519`；只返回公私钥，不要求地址。最终 napi-rs 桥接已通过双算法的真实 Keychain 认证读取与 Ed25519 离线签名核对；当前与历史结果分别见验证记录。

## 目录结构

```text
keychain-wallet/
├── Cargo.toml / Cargo.lock       # Rust workspace，共用依赖锁
├── crates/
│   ├── crypto/                  # 密钥派生与供 Swift 使用的 C ABI
│   └── node/                    # napi-rs 桥接、通信与内存缓存
├── macos/
│   ├── KeychainWallet.xcodeproj/
│   ├── Sources/                 # Swift 界面与 Keychain 访问
│   ├── include/                 # C 头文件
│   ├── Tests/                   # Swift 自检与 Python 配置检查
│   └── …                        # Info.plist、entitlements、xcconfig
├── src/index.ts                 # Node 公开接口与原生模块加载
├── test/                        # Node SDK 集成测试
├── examples/
├── scripts/                     # 仓库级构建、检查入口
├── docs/                        # 签名说明与验证记录
└── package.json
```

`npm run build` 生成 `dist/index.js`、类型声明和 `dist/keychain-wallet.node`。Rust 默认产物在根目录 `target/`，Xcode 产物继续位于 `build/xcode/`，无需更改 SDK 的公开调用或应用查找位置。Swift 静态库使用 workspace 的 `crypto-release` 配置，保留 `panic = "abort"`；Node 原生模块继续使用 `release` 配置。

## 工作方式

1. 打开应用选择 **Create a new wallet** 或 **Import an existing wallet**。创建通过系统安全随机数和 BIP-39 库生成 12 个英文助记词，离线备份并完整重新输入确认后才保存。导入在本机窗口手工输入英文 BIP-39 助记词和可选附加口令。字段遮蔽输入，导入与附加口令字段禁用复制、剪切和粘贴，不接收来自参数、环境变量或文件的秘密。新建备份页提供显式 Copy，只有新建确认框允许显式 Paste / Cmd+V。
2. 选择算法后，导入先校验助记词校验和，再按指定路径派生：secp256k1 比对独立取得的 Ethereum 地址，Ed25519 比对独立取得的 32 字节公钥（`0x` hex）。创建先确认备份。成功后显示名称、算法、公钥和路径；secp256k1 另显示地址。同名条目不会覆盖。
3. 存储目标始终是 Data Protection Keychain，访问组固定为签名应用自己的组。条目创建时绑定 `SecAccessControl(.userPresence)`，可访问性为 `WhenUnlockedThisDeviceOnly`，不启用 iCloud 同步。认证要求位于条目上，代码没有“弹窗认证后读取无保护条目”的替代路径。
4. 业务首次请求密钥时，Node 经 napi-rs 调用 Rust 桥接，桥接启动一个短生命周期原生进程。原生组件新建 `LAContext`，复用窗口设为 0，由 `SecItemCopyMatching` 触发 macOS 系统认证。系统可选择 Touch ID 或设备所有者密码；工具本身不收集 Mac 登录密码。
5. 原生进程只将所选账户的 32 字节私钥写到 Node 创建的 fd 3 私有管道，stdout 仅输出名称、算法、路径和公钥。助记词、BIP-39 根 seed、附加口令不传给 Node；Ed25519 的所选子密钥以 32 字节 seed 表示。进程退出前销毁认证上下文。
6. Rust 为同一 helper／名称／算法／路径组合缓存密钥结果或固定错误码，并串行处理系统认证。同一组合的并发请求共用一次读取；失败也缓存，不会因后续请求静默重试。失败后需要修正原因并重启业务进程。

缓存位于当前 Node 进程加载的同一份 Rust 原生模块内，没有跨进程解锁状态。不同业务进程分别认证；同一助记词的不同派生账户分别认证。worker thread 被显式拒绝，请在主线程取得账户；不要把私钥发给 worker。使用同一个安装路径导入此包；复制并加载另一份原生模块不在去重保证内。认证等待放在 Rust 的阻塞任务池，不占用 Node 的 libuv 文件操作线程池。

## 免费 Personal Team 快速开始

已提供实际工具的 `macos/KeychainWallet.xcodeproj`，可由 Xcode 自动管理免费 Personal Team 的开发签名，无需先手工创建临时 App 或导出签名私钥。具体本机操作见 [Personal Team setup](docs/signing.md)。先在 Xcode 登录 Apple Account，再在目标的 Signing & Capabilities 中选择自己的 Personal Team；项目未填写任何账号或 Team ID。

Xcode 的共享 Run scheme 使用 Release、不挂调试器，按 Command-R 打开创建/导入向导；也可在 Finder 中打开构建的整个 `.app`。`check-signing` 保留为独立的公开签名前置检查命令。本机已选择 Personal Team，并通过签名构建和前置检查。免费签名的有效期和授权以实际 profile 为准；本机已用公开测试钱包完成真实 Keychain 认证读取。

## 运行和签名条件

最低目标为 macOS 14，必须在已登录用户的图形会话中运行。SSH、无图形会话、系统 daemon 和 CI 的单元测试不能证明系统认证可用。Touch ID 非必需，但密码回退必须在实际机器验证。

Apple 的 [TN3137](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains) 明确说明：Data Protection Keychain 访问组从可执行文件的 entitlements 取得，这些权限必须由 provisioning profile 授权；命令行工具需要 app-like bundle 来嵌入 profile，且仍只能在用户上下文使用。不是一个 `swift run` 或 ad hoc `codesign -s -` 就能完成的方案。

原生包结构：

```text
KeychainWallet.app/
  Contents/
    Info.plist
    embedded.provisionprofile
    MacOS/KeychainWallet
    _CodeSignature/...
```

可以使用 Xcode 自动管理签名，或自行准备与 bundle ID 匹配的 macOS 签名证书和 provisioning profile，且 profile 允许相应 `com.apple.application-identifier` 和 `keychain-access-groups`。脚本使用 profile 的 App Identifier Prefix，**不假设它总等于 Team ID**。所签 entitlements 只包含该应用标识、Team ID 和一个私有钥匙串组，不使用通配访问组或任意应用访问权限。

脚本会在新的构建目录中生成包、嵌入 profile、启用 Hardened Runtime 并签名；手动构建脚本没有自动注册 App ID、下载证书、变更凭证或上传公证操作；使用 Xcode 自动签名时，Xcode 会按所选账号管理开发签名资料。原生组件拒绝缺失 profile、未通过代码签名有效性检查、ad hoc 签名、无 Hardened Runtime、错误访问组以及启用调试注入的常见宽松 entitlements。操作系统负责最终的 profile 授权判定，缺失权限时失败，不降级到普通钥匙串。

开发 profile 中“允许调试”的能力不需要签入成品；本脚本不签 `get-task-allow`。证书、profile、设备注册和渠道是否匹配仍需实际构建与运行确认。跨 Mac 分发还涉及 Developer ID 和公证，参见 [Apple 的分发签名说明](https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac/)；本项目不自动发布或公证。

本机已用 Xcode 27.1 完成 Personal Team 签名构建，未读取或导出证书私钥。没有签名身份/profile 时仍会拒绝访问钥匙串；编译和签名前置检查通过不能代替真实 Keychain 认证验证。

## 构建与离线检查

需要 Swift、Rust/Cargo、Node >= 22、npm 和 Command Line Tools 的 Python 3。没有 Rust 工具链时，不提供自写密码学或 JS 派生降级。

```sh
npm ci --ignore-scripts
npm run build
npm run check
npm test
python3 -m unittest discover -s macos/Tests -p 'test_*.py'
bash scripts/check-native.sh

# 签名步骤还需要准备好有效证书/profile。
# 以下三个值均为公开签名配置，不是钱包秘密；替换成你已准备的值。
bash scripts/build-native.sh \
  'Apple Development: Your Name (TEAMID)' \
  '/absolute/path/Wallet.provisionprofile' \
  'com.yourname.KeychainWallet'
```

依赖安装会访问 npm/crates.io。已生成 `package-lock.json` 和根目录 `Cargo.lock`，Node 使用 `npm ci`，Rust 脚本使用 `--locked`。没有 postinstall 钩子。源码项目的 `npm run build` 先编译 Rust 桥接，再生成 TypeScript 产物；只构建桥接可运行 `npm run build:bridge`。`ws` 使用同一主版本的修复版本覆盖（本次锁定 8.22.0）；全量 npm 审计仍报告 ethers v5 开发依赖链的 14 项 low，保留 v5 兼容测试，没有强行替换为 v6。接入层没有 npm 生产依赖；Rust 原生模块使用锁定的 napi-rs、Tokio、serde_json、k256、ed25519-dalek 等依赖。此前 npm 生产依赖审计为 0 项不代表这些 Rust 依赖或业务钱包库已通过安全审计，本次没有重新做依赖审计。

脚本输出新的 `.app` 绝对路径；不要把构建产物移出包单独运行。你可自行把整个包放到稳定位置，Node 接入默认查找 `~/Applications/KeychainWallet.app`、`/Applications/KeychainWallet.app`，然后查找该包自己的本地 Xcode Release 构建。无需在业务代码填写包内可执行文件路径。放在其他位置时可用可选的 `helperPath` 显式指定绝对路径。升级时保留 bundle ID、私有访问组和可授权的签名身份；更换身份可能导致旧条目不可访问，请保留独立离线备份。

## 本机创建、导入和管理

打开 `build/xcode/Build/Products/Release/KeychainWallet.app`（或 Xcode 按 Command-R）：

- **Create a new wallet**：选择算法、输入本地名称。secp256k1 默认使用 `m/44'/60'/0'/0/0`；Ed25519 必须填写完整 hardened 路径，各级都带 `'`。确认是否使用 BIP-39 附加口令，揭示并离线备份 12 个助记词，再从备份完整重新输入（也可通过 Paste recovery phrase 按钮或 Cmd+V 粘贴）。确认成功才写入 Keychain。
- **Import an existing wallet**：选择算法、输入本地名称、派生路径和独立取得的核对值：secp256k1 使用 Ethereum 地址；Ed25519 使用 32 字节公钥的 `0x` hex（不是 base58 等链地址）。再手工输入助记词和附加口令，不匹配时不会保存。
- 成功页显示可供业务使用的名称、算法、公钥和路径；secp256k1 另显示地址。取消任一步骤不会新增条目，同名不会覆盖。界面为英文。

新建钱包的助记词只在创建时的本机备份页面显示，不进入 stdout、日志或文件。用户可点击 Copy recovery phrase 写入系统剪贴板；应用仍运行时会在 60 秒后尝试清除，复制其他内容后不会清除新内容。剪贴板管理器或其他应用可能保留副本，自动清除不保证撤回这些副本。工具不提供日常导出助记词的命令；保留独立离线备份。初期只使用无资金钱包。

以下命令只有公开信息，变量 `WALLET_HELPER` 是应用路径。不要把助记词、私钥或 BIP-39 附加口令写进这个变量或命令。

```sh
WALLET_HELPER='/absolute/path/KeychainWallet.app/Contents/MacOS/KeychainWallet'

# 仅供首次验证的公开 Hardhat 测试账户，永远不要入金。
"$WALLET_HELPER" import demo "m/44'/60'/0'/0/0" \
  '0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266'

# 只列出公开名称；不读秘密。如果系统不允许无交互读取属性，会报错。
"$WALLET_HELPER" list

# 触发系统认证，派生后只输出公开地址，必须匹配预期地址。
"$WALLET_HELPER" address demo "m/44'/60'/0'/0/0" \
  '0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266'

# 仅在你明确需要删除时执行：本机确认窗口 + 系统认证。
"$WALLET_HELPER" delete demo
```

测试钱包在导入窗口手工输入：`test test test test test test test test test test test junk`，并勾选 “I use no BIP-39 passphrase”。这是公开的 Hardhat 开发助记词，只能用于无资金验证。新钱包可在应用向导中生成；导入不自动读取任何来源，不调用剪贴板，不提供日常导出秘密的命令。不要把真实钱包发到聊天。

真实导入时，预期地址或公钥应由你现有钱包或可信离线备份独立取得，不从这个工具的派生结果反填。旧命令行 `import` / `address` 继续仅支持 secp256k1 与 Ethereum 地址，双算法使用图形向导或 Node SDK。secp256k1 只支持路径 `m/44'/60'/account'/change/index`，其中 change 为 0 或 1，索引小于 2³¹。地址逐字节比较，大小写不影响相等性；不宣称对输入执行 EIP-55 校验。

BIP-39 附加口令不是 macOS 登录密码，也不是普通钱包软件的本地解锁密码。工具要求二次输入非空附加口令，或明确勾选空值。附加口令不 trim，空格保留，由 BIP-39 库进行 NFKD 规范化；不同附加口令会得到不同的钱包。它与助记词存于同一认证条目，**因此在本方案中不是独立的第二认证因素**。英文词表外的助记词会拒绝，不能自行换词翻译。

同名新增返回 `ALREADY_EXISTS`，不会覆盖。需要替换时先用新名称导入并核对；删除旧条目是独立的显式操作。工具不能替代离线备份，`ThisDeviceOnly` 与关闭同步不适合作为跨设备恢复机制。

## Node.js / TypeScript 接入

安装/构建这个独立本地包后，可在业务项目安装：`npm install /absolute/path/keychain-wallet`。包为 ESM；CommonJS 使用 `await import('macos-keychain-wallet')`。没有修改现有业务项目。

```ts
import { getKeyPair } from 'macos-keychain-wallet';

const { publicKey, privateKey } = await getKeyPair({
  name: 'demo',
  curve: 'secp256k1',
  index: 0, // 可省略；第一个地址的密钥。
});
```

`curve` 必传，不做算法猜测或默认回退。Ed25519 用法：

```ts
const { publicKey, privateKey } = await getKeyPair({
  name: 'demo',
  curve: 'ed25519',
  path: "m/44'/501'/0'/0'",
});
```

这里只演示 Solana 官方示例中的一种路径；算法本身不决定某条链或钱包的路径。恢复现有账户必须使用原钱包的确切规则，不能保证兼容所有 Ed25519 派生方案（例如其他 Ed25519-BIP32 方案）。同一名称保存的是一份助记词及 BIP-39 附加口令，可分别派生两种算法，不需要重新导入；不同算法的主密钥使用各自标准的域隔离，不复用原始私钥。

只返回两个 `0x` 十六进制字符串，勿记录整个返回对象：

| curve | privateKey | publicKey |
| --- | --- | --- |
| `secp256k1` | 32 字节私钥标量 | 65 字节非压缩公钥，以 `0x04` 开头 |
| `ed25519` | 32 字节私钥 seed | 32 字节公钥 |

Ed25519 的 `privateKey` 是所选子密钥的 seed，不是 BIP-39 根 seed、展开标量或某些 SDK 的 64 字节 keypair。业务接入相应 SDK 时应选用接受 32 字节 seed 的导入方法。

secp256k1 的 `index` 默认为 `0`：`0` 是第一个地址，`1` 是第二个，路径为 `m/44'/60'/0'/0/index`。只接受大于等于 0、小于 2³¹ 的整数；高级用法可传完整 `path`，不能同时指定 `path` 和 `index`。Ed25519 必须显式传 `path`，不接受 `index`；使用 SLIP-0010，路径至少包含一级且每一级都必须 hardened（带 `'`），各级数值小于 2³¹，两种算法路径均最多 128 字节。示例中第一个 Solana 账户之后的路径可用 `m/44'/501'/1'/0'`，前提是它与目标钱包约定一致。

源码分工：`crates/node/src/` 处理验证、应用查找、fd 3 通信、120 秒超时、串行认证、内存缓存和公私钥对应检查；`src/index.ts` 统一保留公开类型、接口及 Node-API 模块加载；薄封装已合并，删除 `src/bridge.ts`。

SDK 不再要求 `address` 或 `expectedAddress`，也不再提供 `createKeychainSession`、viem account 或 ethers Wallet 封装。业务如果需要地址核对、签名或 RPC，自行使用自己的钱包库：

```ts
// viem
const account = privateKeyToAccount(privateKey);

// ethers v5 / v6
const signer = new Wallet(privateKey, provider);
```

完整示例见 `examples/`。工具的职责是保存、认证读取和派生密钥；地址、交易和钱包库对象由业务负责。图形导入保留独立预期地址 / 公钥核对；原有 secp256k1 命令行校验也保留。

`npm run build` 会生成 `dist/keychain-wallet.node`，本地 npm 包会带上它；缺少或无法加载时返回 `KEYCHAIN_WALLET_BRIDGE_NOT_BUILT`。原生模块按当前 Mac 架构构建，换架构或设备时应重新构建；没有添加自动下载二进制或后台构建行为。

`helperPath` 可省略；固定查找当前用户 Applications、系统 Applications、这个 npm 包自己的本地 Xcode Release 构建。不会搜索工作目录、`PATH` 或其他构建副本。都找不到时返回 `KEYCHAIN_WALLET_APP_NOT_FOUND`。非标准安装位置可显式指定 `helperPath`。

同一 helper／名称／算法／最终派生路径在同一 Rust 模块实例中共用认证结果，并发只认证一次；不同进程分别认证，失败保持到进程退出。没有后台服务、跨进程解锁或磁盘缓存。业务拿到私钥后自行管理使用，锁屏不会撤回它。

原生组件只接受一次最多 2 KiB 的公开名称、算法和路径请求，私钥只写到 Node 创建的 fd 3 管道/socketpair；stdout 只有公开元数据。Swift 应用中的 Rust 静态库统一派生密钥与公钥；Node 进程内的 Rust 桥接使用 k256 / ed25519-dalek 复核两者对应关系。原生应用和 SDK 必须一起升级，缺少算法或公钥元数据的旧 helper 会被拒绝。不会从参数、文件、环境变量读取私钥，也没有常规秘密导出命令。单次认证最长等待 120 秒，超时终止子进程并失败。

错误为固定代码，如 `CANCELLED`、`AUTHENTICATION_FAILED`、`NOT_FOUND`、`SIGNING_CONFIGURATION`、`INVALID_HELPER_RESPONSE`、`HELPER_START_FAILED`；不附带原始异常或秘密。取消和密码错误有时映射到同一认证失败码，都会停止，没有 `.env` 回退。

## 安全边界

这套方案减少 `.env`、配置备份或项目文件被扫描时长期秘密泄露的风险，且条目本身限制访问组并要求系统认证。助记词可在认证后短暂进入原生进程，所选私钥会留在 Node 内存和库账户对象中。

它无法防御已控制业务进程的代码、恶意依赖、可读取该进程内存的攻击者、被替换的可信 helper、系统级入侵、键盘记录或屏幕监控。认证窗口显示 helper 身份和所请求账户，不是对 Node 调用者的可信证明；不要批准来源不明的解锁请求。持有同一签名团队/访问组授权的其他受信程序也属于权限边界，签名配置应保持专用。

原生侧禁用 core dump，Rust 对 seed 和派生私钥使用 zeroize，Swift 尽量清除显式字节缓冲；但 Swift String、Foundation、Rust 库内部临时值、JavaScript 字符串、GC、swap 或系统崩溃报告都不能由这个小工具保证彻底擦除。Node 的内存/诊断 dump 和调试器也可能暴露私钥，不要启用包含秘密的诊断采集。没有承诺进程退出后物理内存绝不残留，只承诺不主动持久化解锁状态。

Apple Secure Enclave 的原生 EC 签名能力与 EVM 使用的 secp256k1 不同；此实现并未让 EVM 私钥在 Secure Enclave 内完成原生签名。Keychain 保护存储和访问，交易由 Node 中原有钱包库签名。

已泄露的助记词或私钥不会因为迁入 Keychain 而恢复安全。需要迁移资金和授权时应另行规划；本工具不会自动删除 `.env`、旧文件或钱包，也不会签署/广播真实交易。

## 已验证与待验证

已执行的构建、测试和边界检查见 [验证记录](docs/verification.md)。离线测试与真实系统认证分别记录；公开测试钱包的真实保存、读取和禁止交互读取拒绝均已执行。

已通过签名构建与前置检查、公开测试钱包的真实 Keychain 保存和系统认证读取、两个独立 Node 进程解锁、单进程并发仅启动一个 helper、三种库的离线签名和地址恢复。同组签名测试程序关闭交互后无法读取条目；同名新增和不存在的名称均按预期拒绝。

公开测试条目通过临时签名程序调用生产保存代码创建，随后由正式应用读取；正式图形界面也已完成随机测试钱包创建、Copy → Cmd+V → 确认保存，随后经 Node 认证读取核对地址；公开 Hardhat 助记词图形导入已校验保存。Ed25519 的完整新建和导入也已完成，两个新条目经 napi-rs 认证读取并核对公钥、离线签名；错误公钥被拒绝。仍需分别验证取消/错误密码、Touch ID 与密码回退、锁屏后读取及续签行为。
验证只使用公开测试钱包，无网络 RPC、真实交易签名或广播。即使上述通过，也只能确认测试机器及相应签名配置下的行为，不能证明对任意 macOS 版本和部署方式都成立。

## 依据

- Apple：[钥匙串实现与权限条件](https://developer.apple.com/documentation/technotes/tn3137-on-mac-keychains)、[受限权限的 app-like bundle](https://developer.apple.com/documentation/xcode/signing-a-daemon-with-a-restricted-entitlement)、[钥匙串访问控制](https://developer.apple.com/documentation/security/restricting-keychain-item-accessibility)、[认证上下文及复用说明](https://developer.apple.com/documentation/localauthentication/accessing-keychain-items-with-face-id-or-touch-id)。
- 密码学库：[rust-bip39](https://github.com/rust-bitcoin/rust-bip39)、[bip32 0.5.3 / RustCrypto k256 后端](https://docs.rs/bip32/0.5.3/bip32/)、[RustCrypto Keccak](https://docs.rs/sha3/0.10.8/sha3/)。Ed25519 使用 [ed25519-dalek-bip32 0.3.0](https://docs.rs/ed25519-dalek-bip32/0.3.0/ed25519_dalek_bip32/) 和 [ed25519-dalek 2.2.0](https://docs.rs/ed25519-dalek/2.2.0/ed25519_dalek/)，显式启用 zeroize。遵循 [SLIP-0010](https://github.com/satoshilabs/slips/blob/master/slip-0010.md)、[RFC 8032](https://www.rfc-editor.org/rfc/rfc8032.html)；路径示例参考 [Solana 官方示例](https://solana.com/developers/cookbook/wallets/restore-from-mnemonic)。不自行实现 BIP-39、BIP-32、SLIP-0010、曲线运算或哈希。
- Node 绑定：[napi-rs](https://napi.rs/docs/concepts/async-task)；通过 Node-API 异步调用 Rust，JS 对外仍返回 Promise。
- 业务侧参考：[viem privateKeyToAccount](https://viem.sh/docs/accounts/local/privateKeyToAccount)、[ethers v5 Wallet](https://docs.ethers.org/v5/api/signer/#Wallet)、[ethers v6 Wallet](https://docs.ethers.org/v6/api/wallet/#Wallet)。

界面流程参考 [MetaMask 官方创建与导入入口](https://support.metamask.io/hc/en-us/articles/360015489531) 和 [助记词导入说明](https://support.metamask.io/start/use-an-existing-wallet)。本工具支持英文 BIP-39 助记词、EVM secp256k1 派生与 hardened-only SLIP-0010 Ed25519 派生。
