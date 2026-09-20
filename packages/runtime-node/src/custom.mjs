import { ExecutionError } from './errors.mjs';

/**
 * 宿主自定义能力（自定义 CapabilitySet 的能力）：Node 宿主未知 → 访问得函数、调用即抛。
 * v1 无 proc-macro 定义自定义能力；此处仅保证 `import { custom }` 存在性。
 */
const throwHost = (prop) => async () => {
  throw new ExecutionError(`'custom.${String(prop)}' is not available in Node (host capability)`);
};

export const custom = new Proxy({}, {
  get(_target, prop) {
    const fn = throwHost(prop);
    fn.call = fn;
    return Object.freeze({ call: fn, __name: `custom.${String(prop)}` });
  },
});