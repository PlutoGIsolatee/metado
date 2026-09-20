import { PermissionDeniedError } from './errors.mjs';

const granted = new Set();
const DENY_ALL = Symbol('deny-all');

export function grant(...permissions) {
  for (const p of permissions) granted.add(p);
}

export function grantAll() {
  granted.add(DENY_ALL);
}

export function resetGrants() {
  granted.clear();
}

export function isGranted(permission) {
  return granted.has(DENY_ALL) || granted.has(permission);
}

export function requireGrant(permission) {
  if (!isGranted(permission)) throw new PermissionDeniedError(permission);
  return permission;
}