import { ExecutionError } from './errors.mjs';
import { requireGrant } from './grant.mjs';

/**
 * 宿主自定义能力（自定义 CapabilitySet 的能力）：permission 名 = `custom.<name>`。
 * dispatch(name, params) 按 name 请求对应权限；Node 宿主无真实实现 → 授权后抛 stub。
 */
export const custom = {
  async dispatch(name, params) {
    await requireGrant(`custom.${name}`);
    throw new ExecutionError(`'custom.${name}' is not available in Node (v1 stub)`);
  },
};

/** metado：宿主通用入口，`metado.custom(name, params)` 是 `custom.dispatch` 的别名。 */
export const metado = {
  custom: (name, params) => custom.dispatch(name, params),
};