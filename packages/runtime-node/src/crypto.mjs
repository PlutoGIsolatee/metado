import { stub } from './stub.mjs';

export const crypto = {
  randomBytes: stub('crypto.randomBytes', { sync: true }),
  sha256: stub('crypto.sha256'),
  hmac: stub('crypto.hmac'),
};