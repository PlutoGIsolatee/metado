import { stub } from './stub.mjs';

export const storage = {
  read: stub('storage.read'),
  write: stub('storage.write'),
};