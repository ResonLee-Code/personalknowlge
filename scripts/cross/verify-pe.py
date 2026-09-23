#!/usr/bin/env python3
"""校验 Windows 可执行文件的基本结构（架构、GUI 子系统、资源节）。

用途：在 macOS/Linux 上交叉编译后，确认产物确实是为 Windows 准备的可执行文件，
并且包含图标 / 版本信息 / 清单等资源（由 tauri-build 生成）。
"""

import struct
import sys
from pathlib import Path


def verify(path: Path) -> int:
    data = path.read_bytes()
    problems: list[str] = []

    if data[:2] != b"MZ":
        problems.append("缺少 DOS 头（MZ）")
    pe_offset = struct.unpack_from("<I", data, 0x3C)[0]
    if data[pe_offset : pe_offset + 4] != b"PE\0\0":
        problems.append("缺少 PE 签名")

    machine = struct.unpack_from("<H", data, pe_offset + 4)[0]
    machine_name = {0x8664: "x86-64", 0x14C: "i386", 0xAA64: "arm64"}.get(machine, hex(machine))
    section_count = struct.unpack_from("<H", data, pe_offset + 6)[0]
    optional_size = struct.unpack_from("<H", data, pe_offset + 20)[0]
    optional_offset = pe_offset + 24
    magic = struct.unpack_from("<H", data, optional_offset)[0]
    subsystem = struct.unpack_from("<H", data, optional_offset + 68)[0]
    subsystem_name = {2: "GUI", 3: "Console"}.get(subsystem, str(subsystem))

    section_offset = optional_offset + optional_size
    sections: list[str] = []
    for index in range(section_count):
        base = section_offset + index * 40
        name = data[base : base + 8].split(b"\0")[0].decode("ascii", "replace")
        sections.append(name)

    has_resource = ".rsrc" in sections
    has_product_name = "Personal Knowledge OS".encode("utf-16-le") in data
    has_webview = b"WebView2Loader" in data

    if magic != 0x20B:
        problems.append(f"不是 PE32+（magic={hex(magic)}）")
    if machine != 0x8664:
        problems.append(f"不是 x64 架构（{machine_name}）")
    if subsystem != 2:
        problems.append(f"子系统不是 GUI（{subsystem_name}）")
    if not has_resource:
        problems.append("缺少 .rsrc 资源节（图标/版本信息/清单）")
    if not has_product_name:
        problems.append("资源中未找到产品名称")

    print(f"文件：{path}")
    print(f"  架构      ：{machine_name}")
    print(f"  子系统    ：{subsystem_name}")
    print(f"  节区      ：{', '.join(sections)}")
    print(f"  资源节    ：{'有' if has_resource else '无'}")
    print(f"  产品名称  ：{'有' if has_product_name else '无'}")
    print(f"  WebView2  ：{'引用' if has_webview else '未引用'}")
    print(f"  大小      ：{len(data) / 1024 / 1024:.1f} MB")

    if problems:
        print("校验失败：")
        for problem in problems:
            print(f"  - {problem}")
        return 1
    print("校验通过：可执行文件结构符合 Windows GUI 程序要求。")
    return 0


def main() -> int:
    if len(sys.argv) < 2:
        print("用法：verify-pe.py <exe 路径>")
        return 2
    return verify(Path(sys.argv[1]))


if __name__ == "__main__":
    raise SystemExit(main())
