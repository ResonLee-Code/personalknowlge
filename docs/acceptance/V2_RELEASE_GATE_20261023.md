# V2 当前候选发布门禁

## 候选

- 应用代码提交：`78fe086`
- 候选文档提交：`e20ba04`
- macOS release `.app` CDHash：
  `18b3ea47b6fe87db38b6e8a2aef14e1d41d6f57f`
- 签名：ad-hoc
- 真实工作区：`/Users/reson/MyProjects/knowledge`

## 自动化与脚本

| 检查 | 结果 |
| --- | --- |
| Rust library | 205 passed / 1 ignored |
| Rust 集成测试 | 全部通过 |
| 前端 | 83 文件 / 418 用例 |
| Lint / production build | 通过 |
| Playwright E2E | 7 / 7 |
| 验收脚手架 Node 测试 | 7 / 7 |
| release manifest | 2 / 2 |
| release workflow | 7 / 7 |
| updater 签名 | 有效载荷通过、篡改拒绝、版本绑定通过 |
| `codesign --verify --deep --strict` | 通过 |

`cargo audit` 本机未安装，未擅自修改工具链；CI 仍按仓库固定版本执行。

## 性能

命令：`pnpm bench`

- 1 万检索：平均 **9.88 ms**、p95 **18.15 ms**、recall@1 **100%**、
  recall@5 **100%**；
- 3 万图谱：首屏 / 全量 **843.8 ms**、分类过滤 **326.6 ms**、
  2 跳聚焦 **615.1 ms**；
- 两项性能守护均通过。

## 边界

这些结果证明当前开发候选在本机自动门禁和性能守护上通过，不替代 Windows / Linux 真机、
正式签名、公证、真实外部同步或真实语料人工质量验收。
