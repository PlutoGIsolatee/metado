import { ExecutionError } from './errors.mjs';
import { requireGrant } from './grant.mjs';

/**
 * V1 占位 stub（函数形状，镜像 runtime-export-spec）：
 * 未授权 → 异步形状返回 rejected Promise（PermissionDenied）/ 同步形状同步 throw；
 * 已授权 → stub 抛 "not available in Node"（宿主无真实实现）。
 * @param {string} permission
 * @param {{sync?: boolean}} [opts] 同步形状（log/time.now/crypto.randomBytes）同步 throw
 * @returns {(...args: unknown[]) => unknown}
 */
export function stub(permission, { sync = false } = {}) {
  if (sync) {
    return (..._args) => {
      requireGrant(permission);
      throw new ExecutionError(`'${permission}' is not available in Node (v1 stub)`);
    };
  }
  return async (..._args) => {
    await requireGrant(permission);
    throw new ExecutionError(`'${permission}' is not available in Node (v1 stub)`);
  };
}