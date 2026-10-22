// In a business project: import { Wallet, JsonRpcProvider } from 'ethers' (ethers v6).
import { Wallet, JsonRpcProvider } from 'ethers-v6';
import { getKeyPair } from '../src/index.js';

const { privateKey } = await getKeyPair({ name: 'demo', curve: 'secp256k1' });
const signer = new Wallet(privateKey, new JsonRpcProvider('http://127.0.0.1:8545'));
console.log({ address: signer.address });
