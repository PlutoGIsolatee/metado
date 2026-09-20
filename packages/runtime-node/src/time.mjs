import { stub } from './stub.mjs';

export const timeNow = stub('time.now', 'time.now');
export const timeSleep = stub('time.sleep', 'time.sleep');

export const time = { now: timeNow, sleep: timeSleep };