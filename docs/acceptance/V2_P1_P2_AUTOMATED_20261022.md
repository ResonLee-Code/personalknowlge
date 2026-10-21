# V2.0.x P1 / P2 自动验收记录

## 构建与环境

- 日期：2026-10-22
- 分支：`codex/v2.0-ask`
- 版本：1.3.7（开发线，不是 V2 正式发布号）
- schema：16（新增迁移 016：FTS 4 分片）
- 平台：macOS 27 / arm64 / 24 GB / 15 核

## 自动门禁

| 门禁 | 结果 |
| --- | --- |
| Rust 全量测试 | 通过；library 201 passed / 1 ignored，全部集成测试通过 |
| 前端 Vitest | **80 文件 / 407 用例通过** |
| ESLint | 通过 |
| TypeScript / Vite production build | 通过 |
| Playwright E2E | **5/5 通过**（含 Graph 洞察、Writing diff、Sync 设置） |
| updater 正负验签 | 有效载荷通过、篡改拒绝、版本绑定 `2.0.0` |
| Graph 10 万首屏 | **2866.6 ms**（< 3 s） |
| 检索 1 万 | p95 **18.09 ms**、recall@5 **100%** |
| 检索 10 万（4 分片） | p95 **190.69 ms**、recall@5 **100%**、重开 **1246.09 ms**、RSS **344.64 MB** |
| release 二进制 | **11,833,024 B** |
| macOS `.app` 构建 | ad-hoc 签名成功；公证因无凭据跳过 |
| macOS `.app` 启动冒烟 | 进程存活，迁移 1–16 全部完成，文件监听和窗口恢复成功 |

## P1 / P2 专项

- Graph：近邻证据、重复聚类、桥梁/孤儿/孤岛、只读重复预览测试通过；10 万抽样门禁通过。
- Sync：本地 WebDAV / S3 桩、双设备新增/修改/删除/冲突/合并、附件、索引缓存、
  密文、篡改拒绝、错误口令和离线队列全部通过。
- Scale：迁移 016、四片映射一致性、跨分片检索、分页、后台扫描、虚拟结果列表通过。
- Writing：五模式、逐段引用、来源漂移、目标哈希、diff、确认写入通过。

## 保留人工

见 [V2_P1_P2_MANUAL_ACCEPTANCE.md](../V2_P1_P2_MANUAL_ACCEPTANCE.md)。未完成项包括
200 对近邻标注、真实 WebDAV/S3 与物理双设备、真实模型质量、Developer ID / 公证、
Windows/Linux 真机及签名包升级。
