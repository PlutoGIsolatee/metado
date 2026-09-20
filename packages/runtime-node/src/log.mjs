import { stub } from './stub.mjs';

export const logInfo = stub('log.info', 'log.info');
export const logWarn = stub('log.warn', 'log.warn');
export const logError = stub('log.error', 'log.error');
export const logDebug = stub('log.debug', 'log.debug');

export const log = { info: logInfo, warn: logWarn, error: logError, debug: logDebug };