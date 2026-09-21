import { stub } from './stub.mjs';

export const log = {
  info: stub('log.info', { sync: true }),
  warn: stub('log.warn', { sync: true }),
  error: stub('log.error', { sync: true }),
  debug: stub('log.debug', { sync: true }),
};