import { stub } from './stub.mjs';

export const fileRead = stub('file.read', 'file.read');
export const fileReadText = stub('file.readText', 'file.readText');

export const file = { read: fileRead, readText: fileReadText };