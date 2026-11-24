; Inno Setup script for webby. Build with:
;   iscc /DVersion=0.1.0 /DSourceDir=..\..\dist packaging\windows\webby.iss
; SourceDir must hold webby.exe together with WebKit2.dll and the files
; that ship with it (see scripts\fetch-webkit-windows.ps1).

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "..\..\dist"
#endif

[Setup]
AppId={{B5F0A3C2-6D1E-4B7A-9C48-2E7F1A0D3B65}
AppName=webby
AppVersion={#Version}
AppPublisher=webby
AppPublisherURL=https://github.com/rcdelacruz/webby
DefaultDirName={autopf}\webby
DefaultGroupName=webby
DisableProgramGroupPage=yes
; Installs for the current user without asking for administrator rights.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\icons\webby.ico
UninstallDisplayIcon={app}\webby.ico
OutputDir=..\..\dist-installer
OutputBaseFilename=webby-{#Version}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion
Source: "..\icons\webby.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\webby"; Filename: "{app}\webby.exe"; IconFilename: "{app}\webby.ico"
Name: "{autodesktop}\webby"; Filename: "{app}\webby.exe"; IconFilename: "{app}\webby.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\webby.exe"; Description: "{cm:LaunchProgram,webby}"; Flags: nowait postinstall skipifsilent
