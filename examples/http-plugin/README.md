# HTTP Plugin (Task 6.2)

演示 `http` 能力的授权门控：manifest **不** 声明 http 权限；没有 `--grant` 时
`import { http }` 链接失败（导出不存在，§4.3），带 `--grant=http.get` 之后
导出存在。

```console
mdl build examples/http-plugin --key /tmp/http.key --output /tmp/http.mdl

# 无授权 → load 失败（import 未导出的 http 命名空间）
mdl run /tmp/http.mdl            # error: module evaluation rejected …

# 授权后 → typeof http === "object"、typeof http.get === "function"
mdl run /tmp/http.mdl --grant=http.get
mdl test /tmp/http.mdl --grant=http.get
```