import { ExecutionError } from './errors.mjs';
import { requireGrant } from './grant.mjs';

/**
 * V1 占位 stub：先做授权裁决（未授权 → PermissionDenied），
 * 已授权 → 抛 "not available in Node"（宿主无真实实现）。
 * @param {string} permission
 * @param {string} capability
 * @returns {{readonly __permission: string, call: (...args: unknown[]) => Promise<unknown>}}
 */
export function stub(permission, capability) {
  return Object.freeze({
    __permission: permission,
    async call(..._args) {
      await requireGrant(permission);
      throw new ExecutionError(`'${capability}' is not available in Node (v1 stub)`);
    },
  });
}