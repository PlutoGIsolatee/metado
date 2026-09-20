import { stub } from './stub.mjs';

export const storageRead = stub('storage.read', 'storage.read');
export const storageWrite = stub('storage.write', 'storage.write');

export const storage = { read: storageRead, write: storageWrite };