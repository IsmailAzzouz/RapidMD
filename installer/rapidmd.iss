; Inno Setup Script for RapidMD
; Generates standalone modern Windows installer executable

#define MyAppName "RapidMD"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "RapidMD"
#define MyAppExeName "rapidmd.exe"

[Setup]
AppId={{D37E8F91-7C45-4D1E-8B1A-0E3B8F8A94D2}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
OutputDir=dist
OutputBaseFilename=RapidMD-Setup-v{#MyAppVersion}
SetupIconFile=..\assets\Icon\RMD.ico
UninstallDisplayIcon={app}\{#MyAppExeName}
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequiredOverridesAllowed=dialog commandline
ChangesAssociations=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"
Name: "associate_md"; Description: "Register RapidMD as default viewer for Markdown files (.md, .markdown)"; GroupDescription: "File Associations:"

[Files]
Source: "..\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\assets\Icon\RMD.ico"; DestDir: "{app}\assets\Icon"; Flags: ignoreversion
Source: "..\assets\Icon\RMD.png"; DestDir: "{app}\assets\Icon"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\Icon\RMD.ico"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; IconFilename: "{app}\assets\Icon\RMD.ico"; Tasks: desktopicon

[Registry]
; File association for .md
Root: HKA; Subkey: "Software\Classes\.md"; ValueType: string; ValueName: ""; ValueData: "RapidMD.Document"; Flags: uninsdeletevalue; Tasks: associate_md
Root: HKA; Subkey: "Software\Classes\.markdown"; ValueType: string; ValueName: ""; ValueData: "RapidMD.Document"; Flags: uninsdeletevalue; Tasks: associate_md
Root: HKA; Subkey: "Software\Classes\RapidMD.Document"; ValueType: string; ValueName: ""; ValueData: "Markdown Document"; Flags: uninsdeletekey; Tasks: associate_md
Root: HKA; Subkey: "Software\Classes\RapidMD.Document\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\assets\Icon\RMD.ico,0"; Tasks: associate_md
Root: HKA; Subkey: "Software\Classes\RapidMD.Document\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: associate_md

; Context Menu "Open with RapidMD" for any markdown file
Root: HKA; Subkey: "Software\Classes\*\shell\OpenWithRapidMD"; ValueType: string; ValueName: ""; ValueData: "Open with RapidMD"; Flags: uninsdeletekey; Tasks: associate_md
Root: HKA; Subkey: "Software\Classes\*\shell\OpenWithRapidMD"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\assets\Icon\RMD.ico,0"; Tasks: associate_md
Root: HKA; Subkey: "Software\Classes\*\shell\OpenWithRapidMD\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""; Tasks: associate_md

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent
