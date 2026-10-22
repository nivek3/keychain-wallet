import { createRequire } from 'node:module';
import { homedir } from 'node:os';
import { fileURLToPath } from 'node:url';
import { isMainThread } from 'node:worker_threads';

export type Curve = 'secp256k1' | 'ed25519';
export type WalletSelection = {
  /** Optional absolute executable path; normally discovered by the native bridge. */
  helperPath?: string;
  name: string;
} & (
  /** index defaults to 0; path and index cannot be combined. */
  | { curve: 'secp256k1'; index?: number; path?: string }
  /** SLIP-0010: an explicit hardened-only path is required. */
  | { curve: 'ed25519'; path: string; index?: never }
);

export interface KeyPair {
  /** secp256k1: 65-byte uncompressed key (0x04 prefix). Ed25519: 32-byte public key. */
  publicKey: `0x${string}`;
  /** 32-byte secp256k1 scalar or Ed25519 seed; keep in memory and never log it. */
  privateKey: `0x${string}`;
}


type NativeBridge = { getKeyPair(selection: WalletSelection, packageRoot: string, home: string): Promise<KeyPair> };
const require = createRequire(import.meta.url);
const packageRoot = fileURLToPath(new URL('..', import.meta.url));
let bridge: NativeBridge | undefined;

export async function getKeyPair(selection: WalletSelection): Promise<KeyPair> {
  if (!isMainThread) throw new Error('Use the wallet in the main Node thread');
  if (process.platform !== 'darwin') throw new Error('macOS is required');
  if (!bridge) {
    try { bridge = require('../dist/keychain-wallet.node') as NativeBridge; }
    catch { throw new Error('KEYCHAIN_WALLET_BRIDGE_NOT_BUILT'); }
  }
  try { return await bridge.getKeyPair(selection, packageRoot, homedir()); }
  catch (error) {
    if (error && typeof error === 'object' && 'code' in error && error.code === 'InvalidArg') {
      throw new Error('Invalid wallet selection');
    }
    throw error;
  }
}
