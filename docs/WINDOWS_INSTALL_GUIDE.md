# Windows 安装与使用说明

适用于 Personal Knowledge OS **V1.3.4**（Windows 10 1809 及以上 / Windows 11，x64）。

## 版本状态（重要）

| 项目 | 现状 |
| --- | --- |
| 应用代码 | **V1.3.4**（纵向再压缩 + 原生形态全屏控制条；四处版本号 `1.3.4`） |
| Windows 安装包 | ✅ **V1.3.4 已随 Release 发布**：`Personal.Knowledge.OS_1.3.4_x64-setup.exe`（NSIS）与 `…_x64-portable.zip`；⚠️ **未签名**，且**尚未在真实 Windows 上验收**（本机无 Windows；已做结构校验与哈希核对） |
| macOS 安装包 | ✅ V1.3.4 `.dmg`（**ad-hoc 签名**：首次打开需在 系统设置 → 隐私与安全性 → 仍要打开 放行一次） |
| Linux 安装包 | ✅ V1.3.4 `.deb` / `.rpm` / `.AppImage`（同样随 Release 发布） |

下载与校验（**建议先校验再安装**）：

```powershell
# 从 Release 下载后，在下载目录里核对（PowerShell）
Get-FileHash "Personal.Knowledge.OS_1.3.4_x64-setup.exe" -Algorithm SHA256
# 与 Release 里 SHA256SUMS-windows.txt / SHA256SUMS-all.txt 的同名条目对比
```

> 便携版 zip 里那个 `Personal Knowledge OS.exe` 是**应用本体**（x86-64 GUI，可用
> `python3 scripts/cross/verify-pe.py` 校验）；`setup.exe` 的 NSIS 外壳是 **32 位 stub，属正常**。

本地（macOS）重新打包的命令：

```bash
brew install mingw-w64 nsis                 # 一次性安装交叉编译工具链
cd /path/to/PersonalKnowlge
pnpm build                                  # 前端产物
CI=true ./node_modules/.bin/tauri build \
  --target x86_64-pc-windows-gnu --bundles nsis
# 产物：src-tauri/target/x86_64-pc-windows-gnu/release/bundle/nsis/
#       Personal Knowledge OS_<版本>_x64-setup.exe
```

> 官方标注跨平台编译为 **experimental**（不支持全部特性）。要发布给真实用户，
> 建议在 Windows 机器上执行 `.\scripts\windows\build-windows.ps1`
> 或触发 `.github/workflows/release.yml`（三平台统一流水线，Windows job 在 Windows 运行器上构建），
> 然后按本文「五、验收清单」逐项确认。

## 一、安装前准备

| 项目 | 要求 | 说明 |
| --- | --- | --- |
| 操作系统 | Windows 10 1809+ 或 Windows 11（64 位） | 需要 WebView2 运行时 |
| WebView2 Runtime | 通常已随系统安装 | 若缺失，安装程序会提示并给出下载地址 |
| 磁盘空间 | ≥ 300 MB（含示例与备份） | 笔记正文体积取决于内容 |
| 权限 | 普通用户即可 | 不需要管理员权限（安装到当前用户目录） |

## 二、安装

### 方式 A：安装包（推荐）

1. 取得 `Personal Knowledge OS_1.2.0_x64-setup.exe`（NSIS 安装包）。
2. 双击运行。若出现 Windows SmartScreen 提示（当前构建未做代码签名），
   点击「更多信息」→「仍要运行」。
3. 选择安装范围：
   - **仅为我安装**（默认）：装到 `%LOCALAPPDATA%\Personal Knowledge OS`，无需管理员权限；
   - **为所有用户安装**：装到 `%ProgramFiles%\Personal Knowledge OS`，需要管理员权限。
4. 选择是否创建桌面快捷方式与开始菜单项，点击「安装」。
5. 安装完成后自动启动，也可以从开始菜单打开。

静默安装（适合批量部署）：

```powershell
# 仅当前用户
.\Personal Knowledge OS_1.2.0_x64-setup.exe /S

# 所有用户
.\Personal Knowledge OS_1.2.0_x64-setup.exe /S /allusers

# 指定安装目录
.\Personal Knowledge OS_1.2.0_x64-setup.exe /S /D=C:\Tools\PersonalKnowledgeOS
```

> `/D=` 必须是最后一个参数，且路径不要加引号。

### 方式 B：便携版（免安装）

把 `personal-knowledge-os.exe` 和 `WebView2Loader.dll`（若存在）放到同一个目录，
双击即可运行。数据同样保存在你自己选择的知识库目录中，不写注册表。

### 卸载

「设置 → 应用 → 已安装的应用」中找到 Personal Knowledge OS，点击卸载。
卸载只删除程序文件与快捷方式，**不会删除你的知识库目录**。

## 三、第一次使用

### 1. 创建知识库

启动后是欢迎页：

1. 填写「知识库名称」，例如「我的知识库」；
2. 点击「保存目录」右侧按钮选择目录（或在输入框直接粘贴路径），
   例如 `D:\Knowledge\MyKnowledge`；
3. 点击「创建知识库」。

应用会在该目录下建立如下结构：

```text
MyKnowledge\
├── .knowledge\              元数据与索引（SQLite）
│   ├── database.sqlite
│   ├── config.json
│   ├── backups\             备份包
│   └── logs\app.log         运行日志
├── .trash\                  回收站
├── 00_Inbox\                未分类笔记
├── 01_技术\ 02_工作\ 03_项目\ 04_生活\ 99_Archive\
└── attachments\             附件
```

> 也可以点「打开已有目录」，把已有 Markdown 文件夹直接变成知识库：
> 应用会就地初始化，**不改写你的文件**，只补建 `.knowledge` 与索引。

### 2. 新建第一篇笔记

`Ctrl + N` 或点击左上角「新建笔记」→ 填标题（可选分类、标签、初始内容）→ 创建。

笔记会立刻写入磁盘（默认在 `00_Inbox`），编辑器进入自动保存模式：
停止输入约 1 秒后自动保存，右上角显示「未保存 / 保存中 / 已保存」。

### 3. 打开已有知识库

点击左上角知识库名称 → 在「最近打开」中选择，或点「打开知识库」选择新目录。
最近 20 个知识库会被记住。

## 四、日常使用

### 常用快捷键

| 操作 | 快捷键 |
| --- | --- |
| 新建笔记 | `Ctrl + N` |
| 全局搜索 | `Ctrl + K` |
| 立即保存 | `Ctrl + S` |
| 聚焦编辑器 | `Ctrl + E` |
| 关闭当前笔记 | `Ctrl + W` |
| 删除选中笔记 | `Delete` |

### 组织内容

- **分类**：左侧「分类」树对应磁盘目录，支持子分类；
  重命名分类会同步移动目录与笔记文件；删除分类时可选择把笔记移到 Inbox / 父分类 / 仅取消分类。
- **标签**：在编辑器标题下方的标签栏输入回车即可添加；同名标签不区分大小写；
  删除标签不会删除笔记正文。
- **Inbox**：所有未分类笔记的落脚点，适合先记录后整理。
- **收藏**：点击工具栏星标，可在左侧「收藏」中集中查看。

### 搜索

- `Ctrl + K` 打开全局搜索面板：边输入边出结果，方向键选择，回车打开；
- 左侧「全部笔记」上方也可用标题过滤；
- 「搜索」页面支持分类、标签、收藏与时间筛选，并按相关度或时间排序；
- 中文支持连续词与短语（如「全文搜索」），英文与数字支持前缀匹配（如 `borr` 命中 `borrow`）；
- 结果显示匹配摘要并高亮关键词。

如果搜索长时间异常，可到「设置 → 索引与数据 → 重建索引」重建一次（不会改动笔记文件）。

### 与其它编辑器协作

笔记就是普通 Markdown 文件，可以随时用 VS Code、Typora、Obsidian 等打开修改。
应用会监听目录变化：

- 外部新增 `.md`：自动纳入知识库（若文件里已有 `id` 字段则沿用）；
- 外部修改：自动更新标题、字数与索引；
- 外部删除或移动：笔记标记为「文件缺失」，可在左侧对应入口查看；
- 应用自身写入不会触发重复处理（避免回声）。

### 备份与导出

「设置 → 备份与导出」：

- **立即备份**：在 `.knowledge\backups`（或自定义目录）生成
  `KnowledgeBackup_YYYYMMDD_HHMMSS.zip` 包含：笔记 Markdown、**版本历史**、**模板**、
  **回收站**、附件、数据库快照与配置（研究会话与 AI 审计日志不入包——前者是可丢弃的中间产物，
  后者是行为元数据）；恢复到新目录后仍是**同一个知识库**（沿用原来的 id），
  收藏、回收站这类只存在数据库里的状态也会一起回来；
- **校验**：检查压缩包可打开、清单存在、数据库存在、笔记/附件数量一致、逐文件 SHA-256 一致；
- **恢复**：只允许恢复到**空目录**，避免覆盖现有数据；
- **导出 Markdown**：把知识库导出为普通目录（可选包含附件），可交给其它工具使用。

建议每周备份一次，并把备份目录放在 OneDrive / 移动硬盘等其它位置。

## 五、数据与隐私

- 全部内容保存在你选择的知识库目录中，应用不会上传任何内容，也不需要账号；
- 笔记正文是纯 Markdown，元数据与索引在 `.knowledge\database.sqlite`，
  删除该文件后重新打开知识库会自动重建（会重新扫描 Markdown）；
- 删除笔记先进 `.trash`，可恢复；「永久删除」才会移除文件；
- 保存采用「临时文件 + 原子替换」，写入失败不会破坏原文件；
- 若外部程序修改了同一文件，保存时会提示冲突，由你选择重新加载或覆盖。

## 六、常见问题

**1. 启动后白屏或提示缺少 WebView2**
安装 [Microsoft Edge WebView2 Runtime](https://developer.microsoft.com/microsoft-edge/webview2/)（常青版）后重试。

**2. SmartScreen 或杀毒软件提示未知发布者**
当前安装包未做代码签名。可核对文件哈希后选择「仍要运行」；
生产发布建议购买代码签名证书并配置 `bundle.windows.certificateThumbprint`。

**3. 中文搜索搜不到**
确认笔记已保存（索引会跟随保存更新）；仍不行时到设置里重建索引。

**4. 笔记显示「文件缺失」**
文件被外部删除或移动了。把它放回原路径即可自动恢复；或在列表中选择「永久删除」清理记录。

**5. 想换一台电脑**
把整个知识库目录复制过去，在新机器上「打开已有目录」即可；或使用备份包 + 恢复。

**6. 日志在哪里**
`.knowledge\logs\app.log`（工作区日志）；应用数据目录
`%APPDATA%\PersonalKnowledgeOS\`（最近打开列表与全局设置）。

## 七、从源码构建 Windows 安装包

> 本次交付的安装包即由「交叉编译」方式在 macOS 上生成：
> `dist-windows/Personal Knowledge OS_1.2.0_x64-setup.exe`（NSIS）与
> `dist-windows/Personal Knowledge OS_1.2.0_x64-portable.zip`（便携版）。
> 交叉编译产物未做代码签名，首次运行需要放行 SmartScreen。

本次交付产物的校验值（SHA-256，可在 PowerShell 中用
`Get-FileHash <文件> -Algorithm SHA256` 核对）：

```text
d75b97e535c7ed972122d7dcd981d005d6e39dc3e4823d2a27b09ba5154ad606  Personal Knowledge OS_1.2.0_x64-setup.exe
ea25ed29e5e7e33692cd2c16ab5956d9a4e96aad6ceaa3aa1b86c3187ed47392  Personal Knowledge OS_1.2.0_x64-portable.zip
```

> 注意：交叉编译只能验证产物结构（架构、GUI 子系统、资源节、内部文件清单）
> 与业务逻辑测试（Rust 集成测试 + 前端单元/组件用例 + E2E；数量随迭代变化，此处不写死），
> **无法在 Windows 上做启动冒烟测试**。
> 正式发布前建议在 Windows 机器上按第八节清单验收一次，或直接使用
> `.github/workflows/release.yml` 在 Windows 运行器上构建。

### 在 Windows 上构建（推荐，可产出并验证安装包）

```powershell
# 前置：Node.js 20+、pnpm、Rust（MSVC 工具链）、WebView2
pnpm install
.\scripts\windows\build-windows.ps1            # 生成 NSIS 安装包
```

产物：`src-tauri\target\release\bundle\nsis\Personal Knowledge OS_1.2.0_x64-setup.exe`

### 在 macOS / Linux 上交叉编译

本仓库内置了完整的交叉构建脚本，可在 macOS/Linux 上产出未签名的
`personal-knowledge-os.exe`，并用自建 NSIS 打包出安装包：

```bash
# 1. Windows 目标（官方源不可达时可走镜像）
export RUSTUP_DIST_SERVER=https://mirrors.ustc.edu.cn/rust-static
rustup target add x86_64-pc-windows-gnu

# 2. Zig（提供 clang + mingw-w64 sysroot + dlltool/ar/rc）
curl -L https://ziglang.org/download/0.16.0/zig-aarch64-macos-0.16.0.tar.xz | tar xJ
export ZIG=$PWD/zig-aarch64-macos-0.16.0/zig

# 3. 交叉编译可执行文件
scripts/cross/build-windows.sh

# 4. 校验 PE 结构（架构 / GUI 子系统 / 资源节 / 产品名）
python3 scripts/cross/verify-pe.py \
  src-tauri/target/x86_64-pc-windows-gnu/release/personal-knowledge-os.exe

# 5. 生成安装包与便携版（需要 makensis，见下）
scripts/windows/build-installer.sh
```

安装包与便携版输出到 `dist-windows/`。

**在 macOS 上获得 makensis**（NSIS 官方不提供 macOS 二进制）：

```bash
# 方式一：Homebrew
brew install makensis

# 方式二：自建（本次交付采用的方式）
curl -L -o nsis.tar.bz2 \
  https://mirrors.ustc.edu.cn/debian/pool/main/n/nsis/nsis_3.08.orig.tar.bz2
tar xjf nsis.tar.bz2 && cd nsis-3.08-src
python3 -m pip install scons
python3 -m SCons -Q SKIPSTUBS=all SKIPPLUGINS=all SKIPUTILS=all SKIPMISC=all \
  NSIS_CONFIG_CONST_DATA_PATH=no NSIS_MAX_STRLEN=8192 \
  PREFIX=/tmp/nsis install
# 再把 nsis-common 包中的 Stubs/Include/Plugins 放到 PREFIX 目录下，
# 使 NSISDIR=/tmp/nsis（Debian 的 nsis-common_*.deb 内含 Windows stub 二进制）
NSIS_PATH=/tmp/nsis scripts/windows/build-installer.sh
```

交叉构建涉及的脚本：

| 脚本 | 作用 |
| --- | --- |
| `scripts/cross/build-windows.sh` | 组装交叉工具链并编译 Windows 可执行文件 |
| `scripts/cross/zig-cc-win.sh` | 以 Zig 作为 C 编译器与链接器（含参数归一化） |
| `scripts/cross/zig-rc-win.sh` | 资源编译器（图标/版本信息/清单） |
| `scripts/cross/zig-forward.sh` + `make-tool-shims.sh` | 把 `x86_64-w64-mingw32-*`（dlltool/ar/ranlib/cc）转发给 Zig |
| `scripts/cross/make-win-import-libs.sh` | 生成 GNU 命名的导入库副本（Windows crate / WebView2） |
| `scripts/cross/verify-pe.py` | 校验 PE 结构（x64、GUI 子系统、`.rsrc` 资源节） |
| `scripts/windows/installer.nsi` | 安装脚本（按用户/全用户安装、快捷方式、卸载、WebView2 检测） |
| `scripts/windows/build-installer.sh` | 调用 makensis 生成安装包与便携 ZIP |

### 用 GitHub Actions 自动发布（推荐用于持续交付）

仓库内置统一的 `.github/workflows/release.yml`：**一次 `v*` 标签或手动运行**
就在三个平台分别构建（macOS `.dmg` / Windows NSIS `.exe` + 便携 `.zip` /
Linux `.deb`+`.rpm`+`.AppImage`）并附上各自的 `SHA256SUMS-*`；
推送 `v*` 标签时在校验通过后自动创建 Release。细节见
[RELEASE_PROCESS.md](./RELEASE_PROCESS.md)。

## 八、验证清单（安装后建议逐项确认）

- [ ] 安装后能启动，欢迎页正常显示
- [ ] 能创建知识库，目录中出现 `.knowledge`、`00_Inbox`、`01_技术` 等
- [ ] 能新建笔记，文件出现在磁盘且带 frontmatter（`id` / `title` / `tags`）
- [ ] 停止输入 1 秒后状态变为「已保存」
- [ ] `Ctrl + K` 能搜到刚写入的中文关键词，且摘要高亮
- [ ] 用其它编辑器修改文件后，应用内内容自动更新并弹出提示
- [ ] 删除笔记后在「回收站」能恢复
- [ ] 设置中能创建备份，校验通过
- [ ] 卸载后知识库目录仍然完整保留
