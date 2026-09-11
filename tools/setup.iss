; LcL ImageView 安装包脚本（Inno Setup 7）
; 编译：ISCC.exe tools\setup.iss  →  输出 dist\LcL-ImageView-Setup-v0.2.0-win64.exe
; 免管理员：装到 {localappdata}\Programs，注册表全走 HKCU

#define MyAppName "LcL ImageView"
#define MyAppVersion "0.2.0"
#define MyAppExe "imageview.exe"
#define ThumbClsid "7A3E9B21-4C5D-4E8F-9A6B-1D2C3E4F5A6B"

[Setup]
AppId={{B8F2C1A0-3E4D-4F5A-9B6C-7D8E9F0A1B2C}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=LcL
AppComments=Lightweight game-art image viewer (DDS/PSD/TGA/QOI/HDR/GIF/WebP/APNG)
DefaultDirName={localappdata}\Programs\{#MyAppName}
DefaultGroupName={#MyAppName}
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=LcL-ImageView-Setup-v{#MyAppVersion}-win64
Compression=lzma2/max
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
WizardStyle=modern
UninstallDisplayName={#MyAppName}
CloseApplications=yes

[Files]
Source: "..\target\release\imageview.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\target\release\iv_shell.dll"; DestDir: "{app}"; Flags: ignoreversion restartreplace

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{autoprograms}\卸载 {#MyAppName}"; Filename: "{uninstallexe}"

[Tasks]
Name: "thumbs"; Description: "注册资源管理器缩略图（.dds .tga .psd .qoi .hdr .ppm .pgm .pbm）"
Name: "assoc"; Description: "把上述格式加入 LcL ImageView 的“打开方式”"

[Registry]
; ---- 打开方式（ProgId + OpenWithProgids，不抢默认关联）----
Root: HKCU; Subkey: "Software\Classes\LcL.ImageView.Image"; ValueData: "LcL ImageView Image"; Flags: uninsdeletekey; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\LcL.ImageView.Image\DefaultIcon"; ValueData: "{app}\{#MyAppExe},0"; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\LcL.ImageView.Image\shell\open\command"; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.dds\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.tga\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.psd\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.qoi\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.hdr\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.ppm\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.pgm\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
Root: HKCU; Subkey: "Software\Classes\.pbm\OpenWithProgids"; ValueName: "LcL.ImageView.Image"; ValueData: ""; Tasks: assoc
; ---- 资源管理器缩略图扩展 ----
Root: HKCU; Subkey: "Software\Classes\CLSID\{{{#ThumbClsid}}"; ValueData: "LcL ImageView Thumbnail Provider"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\CLSID\{{{#ThumbClsid}}\InprocServer32"; ValueData: "{app}\iv_shell.dll"; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\CLSID\{{{#ThumbClsid}}\InprocServer32"; ValueName: "ThreadingModel"; ValueData: "Apartment"; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.dds\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.tga\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.psd\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.qoi\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.hdr\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.ppm\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.pgm\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs
Root: HKCU; Subkey: "Software\Classes\.pbm\shellex\{{E357FCCD-A995-4576-B01F-234630154E96}}"; ValueData: "{{{#ThumbClsid}}"; Flags: uninsdeletekey; Tasks: thumbs

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "启动 {#MyAppName}"; Flags: nowait postinstall skipifsilent unchecked

[UninstallRun]
; 卸载后刷新 shell 图标/缩略图缓存
Filename: "ie4uinit.exe"; Parameters: "-show"; Flags: runhidden skipifdoesntexist; RunOnceId: "RefreshIconCache"
