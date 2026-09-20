# Hello Plugin (Task 6.1)

最小插件：`boot` 入口回显问候，`onMessage` 入口处理消息。
验证：`mdl build <dir> && mdl run <file> [--grant …]`

```console
mdl build examples/hello-plugin --key /tmp/hello.key --output /tmp/hello.mdl
mdl run /tmp/hello.mdl
```

权限声明：`log.info`（未请求其他命名空间 → export 仅 log + metado）。