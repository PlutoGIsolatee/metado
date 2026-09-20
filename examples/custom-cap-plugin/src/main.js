import { custom, log } from "@metado/runtime";

// 宿主注册 AlertCapability（custom.alert）后，exported 含 custom 命名空间。
// 未注册宿主下 import 链接失败（本示例在 CLI 宿主按缺能力诊断，见 README）。

export default {
  test() {
    const t = typeof custom;
    return t === "function";
  },
};