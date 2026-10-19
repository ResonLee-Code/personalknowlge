# Linux 安装与使用说明

适用于 Personal Knowledge OS **V1.3.6**（x86_64，glibc 发行版：Ubuntu 22.04+、
Debian 12+、Fedora 38+、Arch 等，需 WebKitGTK 4.1）。

## 版本状态（重要）

| 项目 | 现状 |
| --- | --- |
| 应用代码 | **V1.3.6**（与 macOS / Windows 同一份代码；数据库 schema 13） |
| Linux 安装包 | ✅ **V1.3.6 已随 Release 发布**：`.deb` / `.rpm` / `.AppImage`（由 CI 的 ubuntu-22.04 job 产出；本机无法交叉编译 Linux） |
| macOS 安装包 | ✅ V1.3.6 `.dmg`（ad-hoc 签名，首次打开需「仍要打开」放行一次） |
| Windows 安装包 | ✅ V1.3.6（交叉编译、未签名，待真机验收） |

> 产物与代码已对齐（V1.3.6）。安装前建议先校验：`sha256sum -c SHA256SUMS-linux.txt`
> （Release 里的校验值按**你下载到的文件名**写，在下载目录里直接跑即可）。
> 三平台发布流程与发布后抽验清单见 [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)。

## 一、产出安装包（二选一）

### 方式 A：在 Linux 机器上一条命令构建（推荐用于本地验证）

```bash
git clone <repo> && cd PersonalKnowlge
./scripts/linux/build-linux.sh              # 默认构建 deb + rpm + AppImage
# 或只构建指定格式：
./scripts/linux/build-linux.sh deb appimage
```

脚本会自动安装系统依赖（`libwebkit2gtk-4.1-dev`、`libgtk-3-dev`、
`libayatana-appindicator3-dev`、`librsvg2-dev`、`patchelf` 等），构建完打印每个产物的
大小与 SHA-256。产物位置：

```text
src-tauri/target/release/bundle/deb/*.deb
src-tauri/target/release/bundle/rpm/*.rpm
src-tauri/target/release/bundle/appimage/*.AppImage
```

### 方式 B：用 GitHub Actions 构建（无需本地 Linux）

Actions → **Release (macOS + Windows + Linux)** → Run workflow（只出工件）；
或推送 `v*` 标签（三平台构建 + 自动创建 Release）。
流水线在 `ubuntu-22.04` 上跑 `pnpm test` + `cargo test` 后构建 deb/rpm/AppImage，
工件名 `pkos-linux`（含 `SHA256SUMS-linux.txt`）。详见 [RELEASE_PROCESS.md](./RELEASE_PROCESS.md)。

## 二、安装

### deb（Debian / Ubuntu）

```bash
sudo apt install ./Personal-Knowledge-OS_1.2.0_amd64.deb
personal-knowledge-os            # 或从应用菜单启动
```

### rpm（Fedora / openSUSE）

```bash
sudo rpm -i ./Personal-Knowledge-OS-1.2.0-1.x86_64.rpm
# 或：sudo dnf install ./Personal-Knowledge-OS-1.2.0-1.x86_64.rpm
```

### AppImage（免安装，任意发行版）

```bash
chmod +x Personal-Knowledge-OS_1.2.0_amd64.AppImage
./Personal-Knowledge-OS_1.2.0_amd64.AppImage
```

首次运行若提示缺少依赖（多见于精简发行版），安装：

```bash
# Debian / Ubuntu
sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1 librsvg2-2
# Fedora
sudo dnf install webkit2gtk4.1 libappindicator-gtk3 librsvg2
```

## 三、首次使用

1. 启动后填写**知识库名称**与**保存目录**（如 `~/Documents/MyKnowledge`）；
2. 应用会创建 `.knowledge/`（数据库与索引）、`00_Inbox/`、`attachments/`、`.trash/` 与默认分类；
3. 预置一篇欢迎笔记与一个示例模板（`.knowledge/templates/会议记录.md`）。

## 四、数据与目录

| 内容 | 位置 |
| --- | --- |
| 笔记正文 | 知识库目录下的 `.md` 文件（可直接用任何编辑器修改） |
| 数据库与索引 | `.knowledge/database.sqlite` |
| 附件（粘贴的图片等） | `attachments/YYYY-MM/` |
| 回收站 | `.trash/` |
| 模板 | `.knowledge/templates/*.md` |
| 日志 | `.knowledge/logs/app.log` |
| 全局设置 | `~/.local/share/com.personalknowledge.os/`（最近打开列表等） |

## 五、验收清单（Linux 真机）

| # | 项目 | 预期 |
| --- | --- | --- |
| L1 | 安装与启动 | 三种包任一安装后均能启动；界面显示 1.2.0 |
| L2 | 新建知识库 | 生成上述目录结构；预置欢迎笔记与示例模板 |
| L3 | 记录与自动保存 | 输入 1 秒后显示「已保存」；重启后内容保留 |
| L4 | 中文检索 | 搜索中文短语命中；`SQLite索引` 这类混合写法也能命中 |
| L5 | 图片粘贴 | 落盘 `attachments/YYYY-MM/`，预览内联显示 |
| L6 | 双向链接与关系 | `[[` 补全、chip 跳转、反链面板；关系面板可增删并显示角标 |
| L7 | 图谱 | `/graph` 正常渲染；导出 PNG/SVG 落到 `attachments/` |
| L8 | 导入导出 | Obsidian / Notion 目录导入、md/docx/pdf 导出、ZIP 备份与恢复 |
| L9 | 卸载 | 删除应用（或 `apt remove`）后知识库目录与笔记完整保留 |

> 已在本机（macOS）自动化验证的公共部分（schema 5 → 9 的前向迁移、索引自动重建、
> 3 万篇图谱 811.7 ms、1 万篇导入 4.5 s、四类检索召回 100%）见
> [ACCEPTANCE_V1.2.1.md](./ACCEPTANCE_V1.2.1.md)；上一版产物验收见
> [ACCEPTANCE_V1.2.0.md](./ACCEPTANCE_V1.2.0.md)。
> Linux 侧差异主要是 WebKitGTK 版本与打包格式，需按上表逐项确认。
