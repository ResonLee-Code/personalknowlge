# V2 开发版 Windows / Linux 产物验收

## 范围与边界

- 代码基线：`9504027`
- 版本：`1.3.7`（V2 开发线，不是 V2 正式发布号）
- 宿主：macOS 27.0.1 / arm64
- Windows：Zig 0.17.0 + rustup `x86_64-pc-windows-gnu` + NSIS 3
- Linux：Colima QEMU x86_64 隔离 VM，Ubuntu 22.04
- 本记录验证的是**当前源码生成的安装对象**。它不是 Developer ID / 公证 / Windows
  签名后的 Release 包，也不替代真实物理设备和版本升级验收。

## Windows

| 对象 | 大小 | SHA-256 |
| --- | ---: | --- |
| `Personal Knowledge OS_1.3.7_x64-setup.exe` | 4,610,737 B | `8519ace53c39b916a82299f2584b1f633ef8c70fd7cbf8ce25e554e1e62af581` |
| `Personal Knowledge OS_1.3.7_x64-portable.zip` | 6,138,636 B | `596daa01bc3cdd48ee5529c2b090247c549682662a4a71c9cb939885acd6fa2b` |
| `personal-knowledge-os.exe` | 13,420,032 B | `b7669fb6cf873ed968751dc2f790f5dfa2483c993fd5352afbda44587e75aff8` |

验证结果：

- `verify-pe.py`：x86-64、Windows GUI、PE32+、资源节和产品名存在，引用 WebView2。
- NSIS 外壳为正常的 32 位 stub；`7zz t` 完整通过。
- 安装器内部包含 `personal-knowledge-os.exe`、`WebView2Loader.dll` 与 `uninstall.exe`。
- 从安装器提取的应用本体 SHA-256 与本次 release 构建完全一致。
- 便携 zip 完整通过 `unzip -t`；包内应用本体与 release 构建逐字节一致。
- 当前包未签名，尚未在真实 Windows 上执行 NSIS 安装、升级、卸载、ACL 和 SmartScreen。

## Linux

| 对象 | 大小 | SHA-256 |
| --- | ---: | --- |
| `Personal Knowledge OS_1.3.7_amd64.deb` | 8,729,608 B | `269471b11fa7921cf3b67868a9913ed459d6445737f29c16d974c5822bc9b1a6` |
| `Personal Knowledge OS-1.3.7-1.x86_64.rpm` | 8,730,704 B | `64f882f8b9a4aad57beb26397126698a13b5a665b4616eece7cf56602609b0fd` |
| `Personal Knowledge OS_1.3.7_amd64.AppImage` | 86,538,744 B | `5ebfe875627321c0f8ba97fffc9adf49664ad13382a10825bd37700eb6ad27dd` |

### deb

- `dpkg-deb --info / --contents` 通过：amd64、版本 1.3.7，包含可执行文件、desktop 文件
  与三种尺寸图标。
- 在 Ubuntu 22.04 x86_64 隔离环境中 `dpkg -i` 成功。
- 在 Xvfb 中实际启动到工作区首帧，耗时 `2518 ms`；界面显示 21 篇笔记、3 个分类、
  1 个标签和待复核卡片。
- 截图：[54-linux-ubuntu2204-vm.png](../screenshots/54-linux-ubuntu2204-vm.png)
- `dpkg -r` 后 `/usr/bin/personal-knowledge-os` 与 desktop 文件消失。
- 卸载前后知识库 34 个内容文件哈希完全一致；SQLite `integrity_check=ok`，仍为
  21 篇笔记 / 3 个分类 / 1 个标签。SQLite 主文件因正常 checkpoint 发生字节变化，
  但内容级数据和 Markdown 文件均未丢失。

### rpm

- `rpm -qpi / -qpl / -qpR` 通过：x86_64、版本 1.3.7，包含可执行文件、desktop 与图标。
- `rpm -K` 返回 `digests OK`。
- 当前验证环境是 Ubuntu；rpm 安装与升级仍需在 RPM 系真实发行版上人工执行。

### AppImage

- `--appimage-extract` 验证 `AppRun`、主程序、desktop 和图标存在。
- Tauri AppImage 所需的 `WebKitNetworkProcess`、`WebKitWebProcess` 与
  `libwebkit2gtkinjectedbundle.so` 已包含并设置相对 RPATH。
- 使用 `APPIMAGE_EXTRACT_AND_RUN=1` 在 Xvfb 中实际启动到工作区首帧，耗时 `3247 ms`。
- 首轮自行拼装包曾因缺少 WebKit 辅助进程启动失败；补齐后重新打包并实跑通过，失败包未留作证据。
- 未在启用 FUSE 的物理桌面上双击启动；该步骤继续保留人工。

## 仍保留的人工验收

- Windows：全新用户目录、中文空格路径、普通权限、升级、卸载、便携版、签名包、
  SmartScreen、凭据 ACL。
- Linux：Ubuntu 22.04 / 24.04 物理机 deb 安装、AppImage 直接双击启动、升级与卸载。
- macOS：Developer ID、公证、干净机器冷启动、真实自动更新和失败回滚。
- 三平台 Release 上传后的最终资产名、相对下载 URL、`SHA256SUMS-*` 与 `latest.json`。

权威人工清单：

- [V2 P0 人工清单](../V2_P0_MANUAL_ACCEPTANCE.md)
- [V2 P1 / P2 人工清单](../V2_P1_P2_MANUAL_ACCEPTANCE.md)
