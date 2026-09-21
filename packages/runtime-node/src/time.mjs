import { stub } from './stub.mjs';

export const time = {
  now: stub('time.now', { sync: true }),
  sleep: stub('time.sleep'),
};