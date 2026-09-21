import { PermissionDeniedError } from './errors.mjs';

const granted = new Set();
const DENY_ALL = Symbol('deny-all');

/**
 * 段级权限匹配（与引擎 metado_engine::permission_allows 语义一致）：
 * `*`/`<...>` 段通配任意单段；request 为 pattern 的段前缀即授权；
 * 尾段 `*` 作贪心后缀（覆盖含 `.` 的多段域名/路径）。
 * @param {string} pattern
 * @param {string} request
 * @returns {boolean}
 */
export function patternAllows(pattern, request) {
  const p = pattern.split('.').filter(Boolean);
  const r = request.split('.').filter(Boolean);
  const eq = (a, b) => a === '*' || (a.startsWith('<') && a.endsWith('>')) || a === b;
  if (r.length > p.length) {
    if (p[p.length - 1] !== '*') return false;
    const prefix = p.slice(0, -1);
    if (r.length < prefix.length) return false;
    return prefix.every((pk, i) => eq(pk, r[i]));
  }
  return p.slice(0, r.length).every((pk, i) => eq(pk, r[i]));
}

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
  if (granted.has(DENY_ALL)) return true;
  for (const g of granted) {
    if (patternAllows(g, permission)) return true;
  }
  return false;
}

export function requireGrant(permission) {
  if (!isGranted(permission)) throw new PermissionDeniedError(permission);
  return permission;
}