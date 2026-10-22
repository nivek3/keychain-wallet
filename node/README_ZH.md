# keychain-wallet — Node.js

[English](README.md) | **中文**

基于已签名 `keychain-wallet.app` 的本机 macOS 密钥提供器。napi-rs 桥接在系统认证后只返回所选账户的公私钥，支持 secp256k1 和 SLIP-0010 Ed25519。需要 macOS 14+、Node.js 22+，以及单独构建并正确签名的应用。

当前为源码预览版，npm 版本尚未完成发布验证。现在请按下文构建，再通过 `npm install /absolute/path/keychain-wallet/node` 安装。正式发布后的安装方式：

```sh
npm install keychain-wallet
```

发布脚本会生成包含 Apple Silicon 和 Intel 两种架构的原生模块；实际发布前仍需验证产物。使用正式预编译包时无需 Rust。签名 App 需要单独安装到 `~/Applications` 或 `/Applications`，并先创建或导入钱包。下面的 `trading` 必须替换成 App 中已有的名称，SDK 不会自动创建钱包。

```ts
import { getKeyPair } from 'keychain-wallet';

const { publicKey, privateKey } = await getKeyPair({
  name: 'trading',
  curve: 'secp256k1',
  index: 0,
});
```

`curve` 必传。secp256k1 可传可选 `index`（默认 0）或完整 EVM `path`，两者不能同时使用。Ed25519 必须传各级 hardened 的完整路径，如 `{ name: 'trading', curve: 'ed25519', path: "m/44'/501'/0'/0'" }`，不接受 `index`。恢复现有钱包必须使用原钱包的确切派生规则。

结果均为 `0x` hex。secp256k1 返回 32 字节私钥标量和 33 字节压缩公钥；Ed25519 返回 32 字节子 seed 和 32 字节公钥。不要记录或持久化私钥，业务自行管理签名库。

桥接优先查找 `~/Applications` 和 `/Applications` 中的 `keychain-wallet.app`。源码仓库内还可找到 `build/keychain-wallet.app`；其他位置通过 `helperPath` 指定包内可执行文件的绝对路径。npm 包不包含应用，升级时应用与原生模块应一起构建。包为 ESM，CommonJS 可用 `await import('keychain-wallet')`。

成功或失败结果保留在当前 Node 进程缓存中。只能从主线程调用，不同进程分别认证；删除钱包或锁屏不会撤回已返回的密钥。

## 运行示例

从仓库的 `node/` 目录执行，`trading` 是 App 中已保存的钱包名称：

```sh
node --import tsx examples/ethers-v5.ts trading 2
node --import tsx examples/ethers-v6.ts trading 2
node --import tsx examples/viem.ts trading 2
node --import tsx examples/ed25519.ts trading "m/44'/501'/2'/0'"
```

前三个示例只输出 Ethereum 地址；Ed25519 示例只输出 hex 公钥。`index: 2` 对应 `m/44'/60'/0'/0/2`。Ed25519 的路径由调用方选择，不能把其 seed 传给 ethers `Wallet` 当作对应的 Solana 账户。

支持目标是 Node.js 22+。Bun 的 Node-API 支持可能运行这些示例，但本项目尚未完成 Bun 兼容性验证，不能仅因一次调用成功就视为完整支持。

## 故障排查

| 错误 | 检查项 |
| --- | --- |
| `NOT_FOUND` | 名称必须与 App 中的钱包一致；示例不会创建 `trading` 或 `demo` |
| `KEYCHAIN_WALLET_APP_NOT_FOUND` | 安装完整 App，或指定 `helperPath`；npm 包不包含 App |
| `KEYCHAIN_WALLET_BRIDGE_NOT_BUILT` | 检查原生模块是否构建、架构是否正确，原始加载错误位于 `cause` |
| `SIGNING_CONFIGURATION` | 按签名说明检查证书、profile、bundle ID 和私有访问组 |
| `Invalid wallet selection` | curve 必填；path 与 index 互斥；Ed25519 必须指定 hardened-only path |
| `CANCELLED` / `AUTHENTICATION_FAILED` | 在本机完成认证；重启进程后重试 |

失败结果会缓存在当前进程中，修复问题后需要重新运行。不要将私钥、助记词或附加口令附到报错中。

## 开发与发布

在源码仓库根目录运行：

```sh
pnpm --dir node install --frozen-lockfile --ignore-scripts
pnpm --dir node run build
pnpm --dir node run check
pnpm --dir node test
```

`build` 构建当前 Mac 架构。构建脚本位于根目录 `scripts/build-bridge.sh`；pnpm 管理开发依赖，npm 负责发布，业务项目可使用自己的包管理器。

发布前安装两个 Rust 目标，然后在 `node/` 打包：

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
cd node
npm pack
```

`prepack` 自动重新构建两个架构、合并并检查原生模块，然后生成 JS 和类型声明。任一步骤失败都会停止打包。包内只包含这三个产物、说明和包元数据，没有 Rust 源码、构建脚本或签名资料。

先把生成的 `keychain-wallet-0.1.0.tgz` 安装到一个独立项目，验证加载和接口；确认发布账号拥有包名后，使用 `npm publish ./keychain-wallet-0.1.0.tgz` 发布同一个已检查的文件。每次发布前更新 `package.json` 的版本号。签名 App 由仓库根目录单独构建。

## 许可证

[MIT](LICENSE)
