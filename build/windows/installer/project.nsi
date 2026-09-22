Unicode true

####
## Please note: Template replacements don't work in this file. They are provided with default defines like
## mentioned underneath.
## If the keyword is not defined, "wails_tools.nsh" will populate them with the values from ProjectInfo.
## If they are defined here, "wails_tools.nsh" will not touch them. This allows to use this project.nsi manually
## from outside of Wails for debugging and development of the installer.
##
## For development first make a wails nsis build to populate the "wails_tools.nsh":
## > wails build --target windows/amd64 --nsis
## Then you can call makensis on this file with specifying the path to your binary:
## For a AMD64 only installer:
## > makensis -DARG_WAILS_AMD64_BINARY=..\..\bin\app.exe
## For a ARM64 only installer:
## > makensis -DARG_WAILS_ARM64_BINARY=..\..\bin\app.exe
## For a installer with both architectures:
## > makensis -DARG_WAILS_AMD64_BINARY=..\..\bin\app-amd64.exe -DARG_WAILS_ARM64_BINARY=..\..\bin\app-arm64.exe
####
## The following information is taken from the ProjectInfo file, but they can be overwritten here.
####
## !define INFO_PROJECTNAME    "MyProject" # Default "{{.Name}}"
## !define INFO_COMPANYNAME    "MyCompany" # Default "{{.Info.CompanyName}}"
## !define INFO_PRODUCTNAME    "MyProduct" # Default "{{.Info.ProductName}}"
## !define INFO_PRODUCTVERSION "1.0.0"     # Default "{{.Info.ProductVersion}}"
## !define INFO_COPYRIGHT      "Copyright" # Default "{{.Info.Copyright}}"
###
## !define PRODUCT_EXECUTABLE  "Application.exe"      # Default "${INFO_PROJECTNAME}.exe"
## !define UNINST_KEY_NAME     "UninstKeyInRegistry"  # Default "${INFO_COMPANYNAME}${INFO_PRODUCTNAME}"
####
## !define REQUEST_EXECUTION_LEVEL "admin"            # Default "admin"  see also https://nsis.sourceforge.io/Docs/Chapter4.html
####
## Include the wails tools
####
# 明确产品标识与可执行文件名（须在 wails_tools.nsh 之前 define，避免被默认值覆盖）
!define INFO_PROJECTNAME "LOL助手"
!define INFO_COMPANYNAME "LOL助手"
!define INFO_PRODUCTNAME "LOL助手"
!define PRODUCT_EXECUTABLE "LOL助手.exe"
!define UNINST_KEY_NAME "LOL助手"

!include "wails_tools.nsh"

# 已安装则优先沿用上次目录（InstallLocation；旧版由 .onInit 从 DisplayIcon/UninstallString 反推）
InstallDirRegKey HKLM "${UNINST_KEY}" "InstallLocation"

# The version information for this two must consist of 4 parts
VIProductVersion "${INFO_PRODUCTVERSION}.0"
VIFileVersion    "${INFO_PRODUCTVERSION}.0"

VIAddVersionKey "CompanyName"     "${INFO_COMPANYNAME}"
VIAddVersionKey "FileDescription" "${INFO_PRODUCTNAME} Installer"
VIAddVersionKey "ProductVersion"  "${INFO_PRODUCTVERSION}"
VIAddVersionKey "FileVersion"     "${INFO_PRODUCTVERSION}"
VIAddVersionKey "LegalCopyright"  "${INFO_COPYRIGHT}"
VIAddVersionKey "ProductName"     "${INFO_PRODUCTNAME}"

# Enable HiDPI support. https://nsis.sourceforge.io/Reference/ManifestDPIAware
ManifestDPIAware true

!include "MUI.nsh"
!include "LogicLib.nsh"

!define MUI_ICON "..\icon.ico"
!define MUI_UNICON "..\icon.ico"
# !define MUI_WELCOMEFINISHPAGE_BITMAP "resources\leftimage.bmp" #Include this to add a bitmap on the left side of the Welcome Page. Must be a size of 164x314
!define MUI_FINISHPAGE_NOAUTOCLOSE # Wait on the INSTFILES page so the user can take a look into the details of the installation steps
!define MUI_ABORTWARNING # This will warn the user if they exit from the installer.

!insertmacro MUI_PAGE_WELCOME # Welcome to the installer page.
# !insertmacro MUI_PAGE_LICENSE "resources\eula.txt" # Adds a EULA page to the installer
!insertmacro MUI_PAGE_DIRECTORY # In which folder install page.
!insertmacro MUI_PAGE_INSTFILES # Installing page.
!insertmacro MUI_PAGE_FINISH # Finished installation page.

!insertmacro MUI_UNPAGE_INSTFILES # Uinstalling page

!insertmacro MUI_LANGUAGE "English" # Set the Language of the installer

## The following two statements can be used to sign the installer and the uninstaller. The path to the binaries are provided in %1
#!uninstfinalize 'signtool --file "%1"'
#!finalize 'signtool --file "%1"'

Name "${INFO_PRODUCTNAME}"
OutFile "..\..\bin\${INFO_PROJECTNAME}-${ARCH}-installer.exe" # Name of the installer's file.
!ifdef WAILS_INSTALL_SCOPE
  !if "${WAILS_INSTALL_SCOPE}" == "user"
    InstallDir "$LOCALAPPDATA\Programs\LOL助手"
  !else
    InstallDir "$PROGRAMFILES64\LOL助手"
  !endif
!else
  InstallDir "$PROGRAMFILES64\LOL助手"
!endif # 默认安装到 Program Files\LOL助手
ShowInstDetails show # This will always show the installation details.

# 识别已安装目录并写入 $INSTDIR（供目录页作为默认值）。
# 顺序：InstallLocation → DisplayIcon 父目录 → UninstallString 父目录；HKLM 优先，再 HKCU。
# 注意：!macro 必须在 !insertmacro 之前定义，否则 NSIS 报 macro not found。
!macro DetectPreviousInstallDir
    SetRegView 64
    StrCpy $R9 ""

    ReadRegStr $R9 HKLM "${UNINST_KEY}" "InstallLocation"
    ${If} $R9 == ""
        ReadRegStr $R9 HKCU "${UNINST_KEY}" "InstallLocation"
    ${EndIf}

    ${If} $R9 == ""
        ReadRegStr $R8 HKLM "${UNINST_KEY}" "DisplayIcon"
        ${If} $R8 == ""
            ReadRegStr $R8 HKCU "${UNINST_KEY}" "DisplayIcon"
        ${EndIf}
        ${If} $R8 != ""
            # DisplayIcon 形如 ...\LOL助手.exe 或 ...\LOL助手.exe,0（GetParent 取父目录即可）
            ${GetParent} "$R8" $R9
        ${EndIf}
    ${EndIf}

    ${If} $R9 == ""
        ReadRegStr $R8 HKLM "${UNINST_KEY}" "UninstallString"
        ${If} $R8 == ""
            ReadRegStr $R8 HKCU "${UNINST_KEY}" "UninstallString"
        ${EndIf}
        ${If} $R8 != ""
            # UninstallString 形如 "...\uninstall.exe"（含引号，先剥引号）
            StrCpy $R7 $R8 1
            ${If} $R7 == '"'
                StrCpy $R8 $R8 "" 1
            ${EndIf}
            StrCpy $R7 $R8 1 -1
            ${If} $R7 == '"'
                StrCpy $R8 $R8 -1
            ${EndIf}
            ${GetParent} "$R8" $R9
        ${EndIf}
    ${EndIf}

    ${If} $R9 != ""
        ${If} ${FileExists} "$R9\*.*"
            StrCpy $INSTDIR $R9
        ${EndIf}
    ${EndIf}
!macroend

Function .onInit
   !insertmacro wails.checkArchitecture
   !insertmacro DetectPreviousInstallDir
FunctionEnd

Section
    !insertmacro wails.setShellContext

    !insertmacro wails.webview2runtime

    SetOutPath $INSTDIR

    !insertmacro wails.files

    CreateShortcut "$SMPROGRAMS\${INFO_PRODUCTNAME}.lnk" "$INSTDIR\${PRODUCT_EXECUTABLE}"
    CreateShortCut "$DESKTOP\${INFO_PRODUCTNAME}.lnk" "$INSTDIR\${PRODUCT_EXECUTABLE}"

    !insertmacro wails.associateFiles
    !insertmacro wails.associateCustomProtocols

    !insertmacro wails.writeUninstaller

    # 记录安装目录，下次安装/更新自动沿用
    SetRegView 64
    !ifdef WAILS_INSTALL_SCOPE
      !if "${WAILS_INSTALL_SCOPE}" == "user"
        WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
      !else
        WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
      !endif
    !else
        WriteRegStr HKLM "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
    !endif
SectionEnd

Section "uninstall"
    !insertmacro wails.setShellContext

    RMDir /r "$AppData\${PRODUCT_EXECUTABLE}" # Remove the WebView2 DataPath

    RMDir /r $INSTDIR

    Delete "$SMPROGRAMS\${INFO_PRODUCTNAME}.lnk"
    Delete "$DESKTOP\${INFO_PRODUCTNAME}.lnk"

    !insertmacro wails.unassociateFiles
    !insertmacro wails.unassociateCustomProtocols

    !insertmacro wails.deleteUninstaller
SectionEnd
