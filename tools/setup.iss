; LcL ImageViewer 安装包脚本（Inno Setup 7）
; 编译：ISCC.exe tools\setup.iss  →  输出 dist\LcL-ImageViewer-Setup-v0.3.0-win64.exe
; 免管理员：装到 {localappdata}\Programs，注册表全走 HKCU

#define MyAppName "LcL ImageViewer"
#ifndef MyAppVersion
#define MyAppVersion "0.3.0"
#endif
#ifndef BuildDir
#define BuildDir "..\target\release"
#endif
#define MyAppExe "imageview.exe"
#define MyProgId "LcL.ImageViewer.Image"
#define ThumbClsid "7A3E9B21-4C5D-4E8F-9A6B-1D2C3E4F5A6B"

[Setup]
AppId={{B8F2C1A0-3E4D-4F5A-9B6C-7D8E9F0A1B2C}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
VersionInfoVersion={#MyAppVersion}
VersionInfoProductVersion={#MyAppVersion}
VersionInfoProductName={#MyAppName}
AppPublisher=LcL
AppPublisherURL=https://github.com/csdjk/LcL_ImageViewer
AppSupportURL=https://github.com/csdjk/LcL_ImageViewer/issues
AppUpdatesURL=https://github.com/csdjk/LcL_ImageViewer/releases/latest
SetupIconFile=..\crates\iv-viewer\assets\icon.ico
AppComments=Lightweight game-art image viewer (DDS/PSD/TGA/QOI/HDR/GIF/WebP/APNG)
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=LcL-ImageViewer-Setup-v{#MyAppVersion}-win64
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
UninstallDisplayName={#MyAppName}
CloseApplications=yes
; Refresh Explorer after installing/uninstalling Open With registration.
ChangesAssociations=yes

[Files]
Source: "{#BuildDir}\imageview.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BuildDir}\iv_shell.dll"; DestDir: "{app}"; Flags: ignoreversion restartreplace

Source: "..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\CHANGELOG.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{autoprograms}\卸载 {#MyAppName}"; Filename: "{uninstallexe}"

[Tasks]
Name: "thumbs"; Description: "注册资源管理器缩略图（.dds .tga .psd .qoi .hdr .ppm .pgm .pbm）"
Name: "assoc"; Description: "把支持的图片格式加入 LcL ImageViewer 的“打开方式”，并注册为默认应用候选"

[Registry]
; ValueType is mandatory: the default "none" creates keys but ignores ValueData.
; Keep empty SupportedTypes/OpenWithProgids entries as explicit REG_SZ values.
; ---- 打开方式：Applications\imageview.exe ----
; （打开方式列表的图标/名称/命令；注册后「始终」按钮才可用）
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe"; ValueName: "FriendlyAppName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\DefaultIcon"; ValueData: "{app}\{#MyAppExe},0"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\shell\open\command"; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".png"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".jpg"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".jpeg"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".bmp"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".gif"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".webp"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".ico"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".tif"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".tiff"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".hdr"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".dds"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".psd"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".qoi"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".tga"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".ppm"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".pgm"; ValueData: ""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\Applications\imageview.exe\SupportedTypes"; ValueName: ".pbm"; ValueData: ""; Tasks: assoc
; ---- 打开方式：ProgId + OpenWithProgids（不抢默认关联）----
Root: HKCU; ValueType: string; Subkey: "Software\Classes\{#MyProgId}"; ValueData: "{#MyAppName} Image"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\{#MyProgId}\DefaultIcon"; ValueData: "{app}\{#MyAppExe},0"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\{#MyProgId}\shell\open\command"; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.png\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.jpg\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.jpeg\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.bmp\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.gif\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.webp\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.ico\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.tif\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.tiff\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.hdr\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.dds\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.psd\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.qoi\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.tga\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.ppm\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.pgm\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.pbm\OpenWithProgids"; ValueName: "{#MyProgId}"; ValueData: ""; Flags: uninsdeletevalue; Tasks: assoc
; ---- Capabilities + RegisteredApplications（出现在系统「默认应用」设置页）----
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities"; ValueName: "ApplicationName"; ValueData: "{#MyAppName}"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities"; ValueName: "ApplicationDescription"; ValueData: "轻量级游戏美术看图工具：DDS/PSD/TGA/QOI/HDR/GIF/WebP/APNG"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".png"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".jpg"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".jpeg"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".bmp"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".gif"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".webp"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".ico"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".tif"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".tiff"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".hdr"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".dds"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".psd"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".qoi"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".tga"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".ppm"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".pgm"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\LcL\ImageViewer\Capabilities\FileAssociations"; ValueName: ".pbm"; ValueData: "{#MyProgId}"; Tasks: assoc
Root: HKCU; ValueType: string; Subkey: "Software\RegisteredApplications"; ValueName: "{#MyAppName}"; ValueData: "Software\LcL\ImageViewer\Capabilities"; Flags: uninsdeletevalue; Tasks: assoc
; ---- 资源管理器缩略图扩展 ----
Root: HKCU; ValueType: string; Subkey: "Software\Classes\CLSID\{{{#ThumbClsid}}"; ValueData: "LcL ImageViewer Thumbnail Provider"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\CLSID\{{{#ThumbClsid}}\InprocServer32"; ValueData: "{app}\iv_shell.dll"; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\CLSID\{{{#ThumbClsid}}\InprocServer32"; ValueName: "ThreadingModel"; ValueData: "Apartment"; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.dds\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.tga\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.psd\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.qoi\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.hdr\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.ppm\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.pgm\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; ValueType: string; Subkey: "Software\Classes\.pbm\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "启动 {#MyAppName}"; Flags: nowait postinstall skipifsilent unchecked

[UninstallRun]
; 卸载后刷新 shell 图标/缩略图缓存
Filename: "ie4uinit.exe"; Parameters: "-show"; Flags: runhidden skipifdoesntexist; RunOnceId: "RefreshIconCache"
