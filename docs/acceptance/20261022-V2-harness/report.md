# V2 真机验收脚手架可靠性复测

## 修复

1. 文件断言同时要求“最近生成”和 `grep` 时，必须校验**同一个最新文件**；
   旧实现只验时间就提前返回，旧文件中的正文可能掩盖最新文件未写入的问题。
2. 现场准备通过进程打开的 `<workspace>/.knowledge/database.sqlite` 核验 App 当前工作区，
   不再只凭“主界面存在”就继续执行。

## 自动测试

```bash
node --test scripts/acceptance/lib/*.test.js
```

结果：**4 passed / 0 failed**

- 最新文件优先；
- 旧文件正文不能掩盖最新文件缺失正文；
- SQLite 打开路径可还原 workspace；
- 无关路径与 `-wal/-shm` 不会被误认为数据库。

## 真实误写防护复测

当前 App 打开的是：

```text
/private/tmp/pkos-app-scale-smoke-65888
```

尝试声明目标库为 `/private/tmp/pkos-acceptance`：

```text
✗ 场景核验失败：当前打开的是 /private/tmp/pkos-app-scale-smoke-65888，
  不是验收目标 /private/tmp/pkos-acceptance；为避免写错库，已中止
```

这一步发生在任何 UI 写入前，退出码 1。随后改用真实活动工作区：

```bash
scripts/acceptance/run.sh --only 1 \
  --workspace /private/tmp/pkos-app-scale-smoke-65888 \
  --out /private/tmp/pkos-case1-verified.NhCdb1
```

结果：**10 个自动步骤通过 / 0 失败**，文件证据明确为
“刚生成且正文匹配”。用例 1 仍有一条 `[[` 补全和原生日期控件的人工判断。

## 结论

旧版脚手架确实存在“写错工作区”和“弱文件断言”两类风险；修复后均能在自动测试或真实
App 复测中给出确定结果，不再用旧文件、错误工作区或“主界面存在”冒充通过。
