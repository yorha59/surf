# human 待确认事项（精简版）

- 唯一的人类待办清单；仅记录「需要人类决策」的问题。
- 人类不直接改 `PRD.md` / `Architecture.md`；只在此文件给出决策。
- 仅当前轮正在工作的节点 Agent 可新增问题，必须标注`报告Agent`。
- 问题与决策力求一句话表达，避免冗长。
- Coco/对应 Agent按决策完成处理后，应立即删除该条目；当全部处理完时，仅保留本说明。

问题条目模板（将以下块复制并填写）：

```
- 问题: <一句话描述>
  报告Agent: <agent-id>
  需人类决策: <简述选择/输入>
  人类决策: <由你填写>
```

待人类决策：

```
- 问题: macOS GUI/Tauri 构建阻塞，当前 rustc=1.86.0 低于最低要求 1.88.0
  报告Agent: requirements-manager
  需人类决策: 在本机/CI 将 Rust 稳定版工具链升级至 >=1.88.0；执行命令 `rustup update stable` 并确认 `rustc --version` 显示 >=1.88.0；是否同意作为全仓统一最低版本约束用于后续 GUI/交付构建
  人类决策: 
  你来升级环境
- 问题: GUI-1 人工/集成验收未执行，需在 macOS 上完成端到端验证
  报告Agent: requirements-manager
  需人类决策: 在仓库根目录确认 `workspaces/delivery-runner/release/gui/Surf.app`、`workspaces/delivery-runner/release/gui/dist/` 与 `workspaces/delivery-runner/release/installer/Surf-macos-aarch64.dmg` 存在；在具备图形界面的 macOS 上运行 `workspaces/delivery-runner/release/service/surf-service-macos-x86_64 --service --host 127.0.0.1 --port 1234` 启动服务；通过 DMG 或直接运行 `Surf.app` 启动 GUI，完成 Onboarding（含 Full Disk Access）与基础配置，并确认 GUI 发起的 `fetch("/rpc")` 可连通 `http://127.0.0.1:1234/rpc`；选择一个小目录发起扫描，观察 Treemap 与列表视图与 CLI/服务结果一致；最后在本条目下填写「人类决策」，二选一：① 已执行 GUI-1 人工验收并记录结论；② 暂缓 GUI 发布、以 CLI+服务基线继续
  人类决策: 
```

（此处按上方模板逐条填写；处理完成的条目由 Coco 删除。）
