; Personal Knowledge OS - Windows 安装脚本（NSIS 3.x / Unicode）
;
; 由 scripts/windows/build-installer.sh 或 build-installer.ps1 调用，需传入：
;   -DAPP_VERSION=1.0.0        版本号
;   -DSOURCE_DIR=<目录>        存放 personal-knowledge-os.exe 与 WebView2Loader.dll 的目录
;   -DOUTPUT_DIR=<目录>        安装包输出目录
;   -DICON_FILE=<icon.ico>     安装程序图标（可选）
;
; 特性：
;   - 默认按当前用户安装（无需管理员权限），支持 /allusers 提升安装范围
;   - 创建开始菜单与桌面快捷方式，注册卸载信息
;   - 检测 WebView2 运行时缺失时给出官方下载地址
;   - 卸载仅删除程序文件，不触碰用户的知识库目录

!include "MUI2.nsh"
!include "LogicLib.nsh"
!include "FileFunc.nsh"

!ifndef APP_VERSION
  !define APP_VERSION "1.0.0"
!endif
!ifndef SOURCE_DIR
  !define SOURCE_DIR "..\..\src-tauri\target\release"
!endif
!ifndef OUTPUT_DIR
  !define OUTPUT_DIR "."
!endif

!define APP_NAME "Personal Knowledge OS"
!define APP_EXE "personal-knowledge-os.exe"
!define APP_PUBLISHER "Personal Knowledge OS"
!define APP_REGKEY "Software\PersonalKnowledgeOS"
!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\PersonalKnowledgeOS"
!define WEBVIEW2_CLIENT "{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
!define WEBVIEW2_URL "https://developer.microsoft.com/microsoft-edge/webview2/"

Name "${APP_NAME}"
OutFile "${OUTPUT_DIR}\Personal Knowledge OS_${APP_VERSION}_x64-setup.exe"
Unicode true
; 校验用：-DNO_COMPRESS 可生成未压缩安装包，便于检查内部文件清单
!ifdef NO_COMPRESS
  SetCompressor zlib
!else
  SetCompressor /SOLID lzma
!endif

VIProductVersion "${APP_VERSION}.0"
VIAddVersionKey "ProductName" "${APP_NAME}"
VIAddVersionKey "FileDescription" "${APP_NAME} 安装程序"
VIAddVersionKey "FileVersion" "${APP_VERSION}"
VIAddVersionKey "ProductVersion" "${APP_VERSION}"
VIAddVersionKey "CompanyName" "${APP_PUBLISHER}"
VIAddVersionKey "LegalCopyright" "${APP_PUBLISHER}"

!define MUI_ABORTWARNING
!define MUI_FINISHPAGE_RUN "$INSTDIR\${APP_EXE}"
!define MUI_FINISHPAGE_RUN_TEXT "立即启动 ${APP_NAME}"
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

; 支持 /allusers 安装到 Program Files（需要管理员权限）
!macro setInstallScope
  ${GetParameters} $R0
  ${GetOptions} $R0 "/allusers" $R1
  IfErrors 0 +3
    SetShellVarContext current
    StrCpy $INSTDIR "$LOCALAPPDATA\${APP_NAME}"
  ${If} $R1 != ""
    SetShellVarContext all
    StrCpy $INSTDIR "$PROGRAMFILES64\${APP_NAME}"
    RequestExecutionLevel admin
  ${EndIf}
!macroend

Function .onInit
  StrCpy $INSTDIR "$LOCALAPPDATA\${APP_NAME}"
  SetShellVarContext current
  ClearErrors
  ${GetParameters} $R0
  ${GetOptions} $R0 "/allusers" $R1
  ${IfNot} ${Errors}
  ${AndIf} $R1 != ""
    SetShellVarContext all
    StrCpy $INSTDIR "$PROGRAMFILES64\${APP_NAME}"
  ${EndIf}
FunctionEnd

Section "主程序" SectionMain
  SetOutPath "$INSTDIR"
  File "${SOURCE_DIR}\${APP_EXE}"
  IfFileExists "${SOURCE_DIR}\WebView2Loader.dll" 0 +2
    File "${SOURCE_DIR}\WebView2Loader.dll"

  WriteUninstaller "$INSTDIR\uninstall.exe"

  ; 开始菜单与桌面快捷方式
  CreateDirectory "$SMPROGRAMS\${APP_NAME}"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"
  CreateShortCut "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk" "$INSTDIR\uninstall.exe"
  CreateShortCut "$DESKTOP\${APP_NAME}.lnk" "$INSTDIR\${APP_EXE}"

  ; 卸载信息（设置 → 应用）
  WriteRegStr SHCTX "${APP_REGKEY}" "InstallDir" "$INSTDIR"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayName" "${APP_NAME}"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayVersion" "${APP_VERSION}"
  WriteRegStr SHCTX "${UNINST_KEY}" "Publisher" "${APP_PUBLISHER}"
  WriteRegStr SHCTX "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${APP_EXE}"
  WriteRegStr SHCTX "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
  WriteRegStr SHCTX "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
  WriteRegDWORD SHCTX "${UNINST_KEY}" "NoModify" 1
  WriteRegDWORD SHCTX "${UNINST_KEY}" "NoRepair" 1
  ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
  IntFmt $0 "0x%08X" $0
  WriteRegDWORD SHCTX "${UNINST_KEY}" "EstimatedSize" "$0"

  ; WebView2 运行时检测（缺失时提示官方下载地址）
  ClearErrors
  ReadRegStr $2 HKLM "SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\${WEBVIEW2_CLIENT}" "pv"
  ReadRegStr $3 HKCU "Software\Microsoft\EdgeUpdate\Clients\${WEBVIEW2_CLIENT}" "pv"
  ${If} $2 == ""
  ${AndIf} $3 == ""
    MessageBox MB_OK|MB_ICONINFORMATION \
      "未检测到 Microsoft Edge WebView2 运行时。$\r$\n$\r$\n${APP_NAME} 需要它来显示界面，请先安装（免费）：$\r$\n${WEBVIEW2_URL}$\r$\n$\r$\nWindows 11 与较新的 Windows 10 通常已预装。"
  ${EndIf}
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\${APP_EXE}"
  Delete "$INSTDIR\WebView2Loader.dll"
  Delete "$INSTDIR\uninstall.exe"
  RMDir "$INSTDIR"

  Delete "$SMPROGRAMS\${APP_NAME}\${APP_NAME}.lnk"
  Delete "$SMPROGRAMS\${APP_NAME}\卸载 ${APP_NAME}.lnk"
  RMDir "$SMPROGRAMS\${APP_NAME}"
  Delete "$DESKTOP\${APP_NAME}.lnk"

  DeleteRegKey SHCTX "${UNINST_KEY}"
  DeleteRegKey SHCTX "${APP_REGKEY}"

  ; 说明：用户的知识库目录与 %APPDATA%\PersonalKnowledgeOS 不会删除
SectionEnd
