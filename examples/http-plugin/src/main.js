import { http } from "@metado/runtime";

// 注意：manifest 未声明 http.*，因此默认不导出——本模块在无 --grant 时加载失败。
// （§2 授权模型：未请求的能力导出不存在；v1 http 为占位 stub，返回 undefined。）

export default {
  boot() {
    return typeof http;
  },
  test() {
    return typeof http.get === "function";
  },
};