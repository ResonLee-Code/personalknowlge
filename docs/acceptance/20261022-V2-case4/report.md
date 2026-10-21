# V2 P0 用例 4 复测：大纲栏位与拖拽

## 现场

- 日期：2026-10-22
- 平台：macOS 27 / arm64
- 应用：当前运行的 `Personal Knowledge OS.app`
- 工作区：`/private/tmp/pkos-acceptance`
- 命令：

```bash
scripts/acceptance/run.sh --only 4 \
  --workspace /private/tmp/pkos-acceptance \
  --out /private/tmp/pkos-case4-audit.DH9Jis
```

## 结果

- 自动步骤：**39 通过 / 0 失败**
- 用例整体：🖐 待人工，仅因为最后一条是主观手感
- 机器验证的完整路径：

1. 设置页确认“大纲位置 = 与笔记列表共用一栏”；
2. 在列表栏打开大纲，出现“返回笔记列表”和“钉在正文右侧”；
3. 移到正文右侧后，列表栏恢复，右侧出现“收起大纲”；
4. 收起后出现贴边大纲条；
5. 真实鼠标悬停后展开，出现“钉住大纲”；
6. 钉住后恢复常驻态；
7. 从正文右侧移回列表栏，右侧面板消失；
8. 拖拽列表分栏把手并记录起止坐标。

## 证据文件

原始报告与截图位于：

```text
/private/tmp/pkos-case4-audit.DH9Jis/
```

包含 `report.md`、`report.json` 与 5 张状态截图。该目录是临时验收产物，不进入 Git。

## 保留人工

- [ ] 实际拖拽分栏是否跟手；
- [ ] 贴边大纲悬停展开的动效观感。

这两项必须在统一人工真机验收时由人确认，脚本不替人下结论。
