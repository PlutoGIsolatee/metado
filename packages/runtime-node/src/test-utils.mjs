import { grant, grantAll, resetGrants, isGranted } from './grant.mjs';

/**
 * 权限测试工具（对应 planned `__METADO_TEST__`）：把 grant Set 暴露给测试，
 * 未授权能力必须抛 PermissionDenied；授权后不应再拒，但仍抛 "not available in Node"。
 */
export const __METADO_TEST__ = Object.freeze({
  grant,
  grantAll,
  resetGrants,
  isGranted,
});