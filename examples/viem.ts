import { createWalletClient, http } from 'viem';
import { privateKeyToAccount } from 'viem/accounts';
import { getKeyPair } from '../src/index.js';

// Public test account only. Replace these public settings for your own integration.
const { privateKey } = await getKeyPair({ name: 'demo', curve: 'secp256k1' });
const account = privateKeyToAccount(privateKey);
const client = createWalletClient({ account, transport: http('http://127.0.0.1:8545') });
console.log({ address: client.account.address });
// Creating the client sends no transaction. Do not log the account object.
