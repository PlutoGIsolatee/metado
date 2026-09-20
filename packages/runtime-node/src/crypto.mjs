import { stub } from './stub.mjs';

export const cryptoRandomBytes = stub('crypto.randomBytes', 'crypto.randomBytes');
export const cryptoSha256 = stub('crypto.sha256', 'crypto.sha256');
export const cryptoHmac = stub('crypto.hmac', 'crypto.hmac');

export const crypto = { randomBytes: cryptoRandomBytes, sha256: cryptoSha256, hmac: cryptoHmac };