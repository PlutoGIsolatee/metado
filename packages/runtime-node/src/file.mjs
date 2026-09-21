import { stub } from './stub.mjs';

export const file = {
  read: stub('file.read'),
  stat: stub('file.stat'),
};