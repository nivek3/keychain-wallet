// In a business project: import { Wallet, providers } from 'ethers' (ethers v5).
import { Wallet, providers } from 'ethers-v5';
import { getKeyPair } from '../src/index.js';

const { privateKey } = await getKeyPair({ name: 'demo', curve: 'secp256k1' });
const provider = new providers.JsonRpcProvider('http://127.0.0.1:8545');
const signer = new Wallet(privateKey, provider);
console.log({ address: signer.address });
// Keep the existing business code that consumes a Wallet / Signer.
