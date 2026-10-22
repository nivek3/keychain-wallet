# 本机验证记录

最近更新：2026-10-22（北京时间）；下方保留 2026-10-21 的历史验证记录。仅使用本机和无资金测试钱包，没有连接 RPC、签署真实交易或广播交易。

## 2026-10-22 新建备份确认与界面更新

- 界面按用户选择调整为 macOS 原生极简风格：系统字体、紧凑表单、浅灰背景、蓝色主按钮，Back / Cancel 与 Continue / Save 位于同一行，不再使用整行巨型按钮。界面仍由 egui 绘制，并非 AppKit 原生控件；没有改变密钥接口或认证机制。
- 最终界面版本的 Rust workspace 测试 **25/25**（app 12 + 桥接 6 + 密码学 7）、Python 配置测试 **4/4** 通过。新增系统字体与最小窗口表单渲染检查，约束按钮尺寸和可见范围。SDK Release 构建通过；最终 TypeScript 检查、Node 集成测试 **10/10**、Xcode Release 构建、`codesign --verify --strict` 和 `check-signing` 通过。
- 在实际签名应用中，新建 secp256k1 无资金条目 `rust-create-secp-20261022`，默认路径 `m/44'/60'/0'/0/0`。未勾选空附加口令时无法继续；错误备份确认被拒绝，独立 SDK 查询得到 `NOT_FOUND`，确认此时未写入 Keychain。取消后重新生成，完成揭示 12 词、Copy、确认页 Paste 按钮、Confirm and save，显示 `Wallet created`。
- 在最终紧凑界面中，新建 Ed25519 无资金条目 `rust-create-ed-20261022`，路径 `m/44'/501'/0'/0'`，明确选择空附加口令。实际完成生成、揭示 12 词、Copy、聚焦确认输入框、Cmd+V、Confirm and save，显示 `Wallet created`。
- 通过正式 Node SDK，分别对两个新条目完成真实 macOS 系统认证读取。secp256k1 公钥和地址与创建成功页一致，独立 ethers 离线消息签名验证通过；Ed25519 公钥与成功页一致，独立 Node/OpenSSL 从所选 32 字节 seed 重建公钥并完成离线签名验证。只输出 `PASS_NEW_WALLET_SECP256K1_AUTH_AND_OFFLINE_SIGNATURE` 和 `PASS_NEW_WALLET_ED25519_AUTH_AND_OFFLINE_SIGNATURE`。
- 随机助记词未输出到聊天、日志或文件；备份页检查在本机界面控制运行时过滤，仅返回步骤、按钮和词数。私钥不输出或持久化。测试只验证应用内复制确认，不替代长期离线备份；这两个随机测试钱包均不应入金。
- 本次没有 RPC、真实资金交易，没有删除或覆盖钱包。未重跑取消认证、错误密码、Touch ID / 密码回退、锁屏读取、profile 续签或手工 profile 构建；历史结果不能代替本次验证。个人 Team 只保留在本机，不随代码提交。

## 2026-10-22 Rust egui/eframe 迁移

- 新增 `crates/app`：egui/eframe 界面、Apple Security / LocalAuthentication 绑定、创建 / 导入 / 备份确认和既有 helper 命令均由 Rust 实现。原 Swift 源码、头文件与 Swift 测试移出当前源码树，本机备份位于忽略的 `build/legacy/swift-app/`。
- 保留 bundle ID、私有访问组、服务名 `local.keychain-wallet.mnemonic.v1` 与 version 1 JSON 格式；Keychain 查询继续指定 Data Protection、禁止同步，创建使用 `WhenUnlockedThisDeviceOnly` 与 `userPresence`，不会覆盖同名条目。读取使用新的 LAContext、复用时间 0，由 SecItemCopyMatching 触发认证；没有提前认证再读无保护条目的替代路径。
- 迁移签名守卫、Hardened Runtime / ad hoc / 调试例外拒绝、core dump 禁用、固定错误码与 panic 输出限制。窗口激活的秘密输入阶段启用 Secure Event Input；清空输入撤销历史、不持久化 UI，原生按键入口阻止 winit 自动读剪贴板，只有新建确认框显式粘贴。Copy 保留 60 秒清理与保护后续新剪贴板内容的规则。
- Node 的 TypeScript 入口、Rust 桥接和公开接口未改；stdin 公共 JSON 仍限制 2 KiB，fd 3 必须是 pipe/socket，私钥只写此通道，stdout 只含公开元数据。并发、缓存、超时和公私钥对应检查继续由原桥接负责。
- Xcode 已构建并签名实际 Rust 可执行文件，`codesign --verify --strict` 与 `check-signing` 通过。依赖列表确认应用不再链接 Swift 运行库，签名访问组与迁移前一致；本机 Personal Team 设置保留，不提交个人配置。
- SDK Release 构建、TypeScript 检查与 Node 集成测试 **10/10** 通过；Rust workspace 测试 **24/24**（app 11 + 桥接 6 + 密码学 7）、Python 配置测试 **4/4** 通过。测试覆盖两种曲线、旧存储 JSON、错误公钥 / 路径、系统随机生成 12 词但不落盘、备份不完整禁止保存、签名约束、认证上下文配置、剪贴板清理、请求大小与通道类型。
- 实际启动新的 egui 窗口，经图形导入将公开 Hardhat 助记词保存为无资金测试条目 `rust-ui-import-20261022`，选择 Ed25519 路径 `m/44'/501'/0'/0'` 和空附加口令；公钥匹配独立参考，显示 `Wallet imported`。没有读取或显示随机钱包助记词。
- 用户在本机完成两次系统认证；正式 Node SDK 经新 Rust 应用读取旧公开条目 `ui-import-20261021` 的 secp256k1 密钥，独立 ethers 地址核对通过；读取新版图形导入条目的 Ed25519 密钥，独立 Node/OpenSSL 核对公钥及离线签名通过。仅输出 `PASS_RUST_APP_LEGACY_KEYCHAIN_SECP256K1` 和 `PASS_RUST_APP_GUI_IMPORT_ED25519_OFFLINE_SIGNATURE`，未输出或保存密钥。
- 迁移阶段当时未重跑完整随机钱包图形备份 / 确认（后续验证见上节）、取消或错误密码、Touch ID / 密码回退、锁屏读取、续签和无交互真实数据读取拒绝；相应自动化配置检查与旧版实际记录不能替代新版逐项人工验证。没有 RPC、真实资金签名或广播，没有删除或覆盖任何现有钱包。该阶段尚未提交，后续验证与界面更新合并提交。

## 2026-10-22 目录整理

- Rust 源码归入 `crates/crypto`、`crates/node`，新增根 workspace 并共用 `Cargo.lock`。没有引入原锁文件之外的新依赖版本；共用的 `getrandom` 统一到原 Node 锁文件已有的 0.2.17。
- Swift 源码、Xcode 工程、头文件、配置与原生测试归入 `macos/`；Node 入口与测试仍保留在根目录的 `src/`、`test/`，根 npm 包的安装方式不变。
- Node 原生模块输出改为 `dist/keychain-wallet.node`，与 JS / 类型声明一起打包。签名与验证文档移入 `docs/`，原缓存和旧源码 ZIP 保留在忽略的 `build/legacy/`。
- 同步更新 Xcode 源码与构建阶段路径、脚本、测试和文档。静态库继续使用 abort-on-panic 构建，Node 模块继续使用 unwind 构建；未改密钥派生、Keychain 访问组、认证与剪贴板逻辑。
- 本次 SDK / napi-rs Release 构建、TypeScript 检查通过；Node 集成测试 **10/10**、Rust 单元测试 **13/13**（桥接 6 + 密码学 7）、Python 配置测试 **4/4** 全部通过；Swift/Rust 公开向量检查和 Swift 剪贴板自检通过。
- 移动后的 Xcode Personal Team Release 构建、`codesign --verify --strict`、应用 `check-signing` 全部通过。核对待提交的 Xcode 工程与本机构建工程，除未提交的个人 Team 设置外配置一致；迁移前后的 Rust / Swift 实现内容一致。
- 实际执行 `npm pack`，将生成的包安装到独立临时 Node 项目，用公开 RFC 8032 测试 helper 完成密钥返回与公私钥核对；包内只有 JS、类型声明、`.node`、README 和 package.json，未发布 npm 包。
- 本次未重新操作图形创建 / 导入或真实 Keychain 认证读取，未运行独立的手工 profile 构建分支；下方这些操作的记录来自 2026-10-21，不能视为本次重新执行。

## 当前版本：双算法与 napi-rs 桥接

`getKeyPair` 必须显式传 `curve`：`secp256k1` 可用 `index`（默认 0）或完整 EVM 路径；`ed25519` 必须传 SLIP-0010 hardened-only 路径，不接受 `index`。两者仅返回 `publicKey` / `privateKey` 的 `0x` hex。Ed25519 私钥表示所选子密钥的 32 字节 seed，公钥 32 字节；secp256k1 私钥 32 字节、公钥为 65 字节非压缩格式。

- 已将参数验证、应用查找、进程通信、串行认证、缓存、超时与公私钥复核移入 `crates/node`；必要的平台检查与 napi-rs 模块加载已合并进 `src/index.ts`，删除 `src/bridge.ts`；公开调用仍返回 Promise。
- SDK / napi-rs Release 构建及 TypeScript 检查通过；Node 集成测试 **10/10** 通过，使用真正加载的 Rust 原生模块和公开测试 helper。
- Node 检查覆盖两种算法、公私钥对应、RFC 8032 公开签名向量、零 Ed25519 seed、不同路径、并发单次认证、独立进程、固定错误码、失败缓存、短/超长私钥、stdout/stderr 大小限制、元数据不符，以及等待认证时 Node 文件操作仍可完成。
- Rust 桥接单元测试 **6/6** 通过：参数与路径边界、默认索引、用户应用优先、显式错误不回退、可执行文件路径规范化、公钥/元数据核对、超时及继承管道、失败不重试。
- Rust 密码学单元测试 **7/7** 通过，包括 SLIP-0010 官方 Ed25519 master / 多级 hardened 子密钥向量、原有 secp256k1 / BIP-39 向量、FFI 公钥长度与错误输出清零。
- Swift/Rust 跨语言检查通过：两种 Ed25519 路径、公钥比对、附加口令、Unicode NFKD、错误路径与错误公钥，以及既有 EVM 检查；公钥参考值由公开 Hardhat 助记词经独立 Node HMAC-SHA512 / OpenSSL 实现计算。
- Swift 剪贴板自检通过；未扩大既有剪贴板授权范围。
- Xcode Personal Team Release 构建、`codesign --verify --strict` 和 `check-signing` 通过，使用原签名身份、bundle ID 与访问组，没有导出私钥或修改信任设置。
- 完整 Ed25519 图形创建与导入均已通过，详见下方本次记录；另验证非 hardened 路径被拒绝、错误公钥无法导入。
- `npm pack --dry-run` 确认包内包含 `.node` 原生模块、JS / 类型声明与 README，没有钱包数据、证书或 profile；仅检查清单，未发布。
- 最终 napi-rs / Rust 桥接已用同一个公开 Hardhat 测试条目 `ui-import-20261021` 完成两次真实系统认证：secp256k1 默认 index / index 0 并发取钥相同，独立 ethers 核对到预期地址；Ed25519 路径 `m/44'/501'/0'/0'` 的公钥与独立参考一致，离线消息签名验证成功。只输出 `PASS_RUST_BRIDGE_REAL_SECP256K1` 与 `PASS_RUST_BRIDGE_REAL_ED25519_AND_OFFLINE_SIGNATURE`，没有输出或持久化任何密钥。并发仅一次启动的精确计数由真实 Rust 模块加公开 mock helper 的集成测试验证，不靠真实认证检查脚本的启动计数推断。

本机无 RPC 或真实资金交易；没有删除或覆盖 Keychain 条目。下文为历史阶段证据，测试数量与接口形态不代表本版本。

## 本次完整 Ed25519 创建与导入验证

- 在正式签名应用中创建无资金测试条目 `ed-create-20261021`，算法 `ed25519`，路径 `m/44'/501'/0'/0'`，明确选择空 BIP-39 附加口令。实际完成生成、揭示备份、应用内 Copy、仅新建确认框的 Paste、确认与 Keychain 保存，显示 `Wallet created`。
- 第一次备份确认不匹配时显示拒绝，未保存条目；取消后重新创建，在 60 秒剪贴板有效期内完成复制与粘贴，确认成功。未修改安全输入或校验逻辑；助记词未输出到聊天、日志或文件。
- 图形导入使用固定公开 Hardhat 助记词和空附加口令。先以全零错误公钥尝试，显示预期 Ed25519 公钥不匹配且未保存；取消该次尝试后，用独立参考公钥重新导入为 `ed-import-20261021`，显示 `Wallet imported`。
- 正式 Node SDK 经最终 napi-rs 桥接，对这两个新条目分别触发真实系统认证读取。公钥与各自创建/导入成功页匹配，32 字节私钥 seed 可由独立 Node/OpenSSL 实现完成离线消息签名，公钥验证通过。脚本只输出 `PASS_AUTHENTICATED_ED25519_CREATE` / `PASS_AUTHENTICATED_ED25519_IMPORT`。
- 合并删除 `src/bridge.ts` 后，重新执行 SDK / napi-rs 构建、TypeScript 检查和 Node 集成测试，10/10 通过；源码与构建目录中均不再保留旧桥接入口。
- 本次没有删除或覆盖任何钱包，没有连接 RPC 或签署/广播交易。随机生成的测试钱包未建立长期离线恢复备份，不应入金。

## 历史：前一版本仅返回密钥的接口验证

`getKeyPair({ name, index? })` 只返回 `publicKey` / `privateKey`。SDK 构建、类型检查、15 项离线测试、Personal Team 原生构建与新接口真实系统认证取钥均已通过。

- 只传名称，默认 index 0：公私钥格式正确，ethers 独立构造后得到预期公开 Hardhat 地址。
- 默认 index 与显式 index 0 并发调用：实际只启动一个 helper。
- index 1：重新认证后得到不同密钥，ethers 独立构造得到第二个预期公开地址；公钥与私钥对应。
- 真实检查没有把任何公私钥输出到日志或文件，没有连接 RPC 或发送交易。
- `codesign --verify --strict` 在受限执行环境曾返回信任查询错误，在当前用户的可访问信任环境复核通过；没有修改证书或信任设置。

以下保留此前图形创建、导入与 Keychain 访问控制的实际记录。

## 前一阶段实际结果

Personal Team 签名构建、真实 Data Protection Keychain 保存与系统认证读取、Node 私有管道接入，以及 viem / ethers v5 / ethers v6 离线消息签名已通过。

已实际操作正式图形界面：新建 `ui-test-20261021`，从同一备份页点击 Copy，再通过 Cmd+V 填入确认框，校验并保存成功。随后正式 Node 接入通过真实系统认证读取该条目，地址与创建成功页一致，三种库离线签名均验证成功。

另通过图形导入向导手工输入公开 Hardhat 助记词，校验预期地址并保存为 `ui-import-20261021`。此前临时签名程序创建的固定公开测试条目还用于认证约束和访问组负向检查。名称 `peng` 此前实际查询为 `NOT_FOUND`，未把旧成功页报告当成已保存的证据。

## 已执行结果

| 检查 | 结果与范围 |
| --- | --- |
| Personal Team Xcode Release 构建 | 成功，完整应用含嵌入 profile |
| `codesign --verify --strict` | 通过；已检查签名应用只有应用标识、Team 标识和一个专用 Keychain 组，无调试权限 |
| 正式应用 `check-signing` | 输出 `SIGNING_CONFIGURATION_OK` |
| Rust 单元测试 | 5/5 通过：BIP-39 公开熵向量、生成词校验和与熵往返、Hardhat 地址、TREZOR seed、Unicode NFKD、错误输入、FFI 清零 |
| Swift/Rust 原生自检 | 通过：生成公开熵对应助记词与独立参考地址、路径约束、预期地址、附加口令和字节缓冲清零 |
| Python 项目配置测试 | 4/4 通过：profile 授权边界和无调试器的 Release 配置 |
| 真实 Keychain 新增 | 成功，固定公开 Hardhat 测试条目；使用生产 `Vault.insert`、`.userPresence`、`WhenUnlockedThisDeviceOnly`、非同步私有组 |
| 同组、同签名授权程序禁止交互读取 | 新 `LAContext.interactionNotAllowed = true` 读取数据被拒绝，固定认证失败；没有预先调用 `evaluatePolicy` |
| 无访问组授权程序读取 | 返回 `errSecMissingEntitlement`，没有数据，禁止交互 |
| 同名再次新增 | `ALREADY_EXISTS`，无 stdout，不覆盖原条目 |
| 新建 UI → Copy → Cmd+V → 确认保存 | 实际操作通过，助记词未输出到聊天、日志或文件 |
| Copy 清理自检 | 独立临时剪贴板测试通过：复制、定时清除、保留后续新内容、重复复制计时 |
| 图形导入及重新认证读取 | 手工输入公开 Hardhat 助记词，校验地址并保存；正式应用再次触发系统认证，返回匹配的名称、路径和公开地址 |
| 正式应用读取不存在的名称 | `NOT_FOUND`，无 stdout |
| 正式 Node 接入与系统认证 | 成功通过正式应用 `--bridge`，私钥仅经过 fd 3 管道/socketpair |
| viem / ethers v5 / ethers v6 | 三个真实库账户地址均匹配公开预期地址，离线消息签名恢复到同一地址 |
| 同进程并发与后续缓存 | 实测三个并发库请求和一次后续请求，仅启动一次正式 helper |
| 独立 Node 进程 | 两个独立进程分别完成一次正式解锁，未使用共享解锁进程或磁盘缓存 |

前一版本接口收敛为 `getKeyPair({ name, index? })`，只返回 `publicKey` / `privateKey`。不再要求地址，也不再包装 viem / ethers 对象。公钥为 65 字节未压缩 secp256k1 公钥，私钥为 32 字节，均使用 `0x` 十六进制字符串。正常调用不需要填写应用路径或完整派生路径。

SDK 构建、TypeScript 类型检查通过，Node 单元测试 **15/15 通过**，覆盖公私钥一致、第二个索引、非法私钥标量、默认应用位置、路径/索引边界、并发去重与失败不重复认证。测试还由业务侧自行构造 viem / ethers 对象并核对公开测试账户，无 SDK 专用库封装。

原生桥接请求只含公开名称和路径，新接口两个索引的真实认证取钥已完成。图形导入与原生命令行的独立预期地址检查保留。业务如需地址匹配或签名，自行使用自己的钱包库处理。

生产依赖此前审计 0 项；保留 ethers v5 兼容测试的开发依赖链此前报告 14 项 low，本次未重复审计。

当前 profile 实际到期时间：2026-10-28 22:20:12（北京时间）。免费 Personal Team 需要按实际有效期续签构建；保留 bundle ID 和访问组，不删除旧钱包条目。

## 测试数据与处理

公开测试名称：`public-test-20261021-a15e8e`。

公开 Hardhat 预期地址：`0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266`；路径 `m/44'/60'/0'/0/0`。

随机生成的 UI 测试钱包 `ui-test-20261021` 的公开地址为 `0x0a5e65190839dd9da041ff38204da706abfcedb9`，仅用于界面与接入验证，未建立可用于长期恢复的独立离线备份。所有测试条目都不要入金。公开 Hardhat 开发钱包的助记词本来就是公开的。验证程序只包含固定公开测试向量，不接受真实秘密，不提供日常秘密导出入口。临时测试程序放在构建工作区/系统临时目录，不加入发布产品。未删除任何 Keychain 钱包。

## 剪贴板范围

新建确认框已获用户明确授权并通过实际 Cmd+V 和保存验证；Paste 按钮已验证可以填入安全输入框。导入字段仍禁止粘贴。剪贴板自动清除只在应用运行时尝试，不保证其他应用或剪贴板管理器删除副本。

## 尚未验证

- 用户取消、密码错误、Touch ID 与密码回退各自的具体系统行为；不能从一次认证成功推断所有方式都验证过。
- 锁屏后新进程读取、profile 过期续签及其他 macOS / 设备上的运行行为。
- 真实资金和交易。Node 解锁后的私钥仍在业务进程内存中，不能防御被控制的业务代码；显式缓冲清零不保证 egui / Foundation / JavaScript 的全部副本被擦除。

## 复跑离线检查

```sh
npm run build
npm run check
npm test
python3 -m unittest discover -s macos/Tests -p 'test_*.py'
bash scripts/check-native.sh
```

真实系统认证检查需已签名应用、当前用户图形会话和用户在本机完成认证，不能用模拟测试替代。
