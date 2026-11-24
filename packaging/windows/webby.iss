; Inno Setup script for Webby. Build with:
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
AppName=Webby
AppVersion={#Version}
AppPublisher=Webby
AppPublisherURL=https://github.com/rcdelacruz/webby
DefaultDirName={autopf}\Webby
DefaultGroupName=Webby
DisableProgramGroupPage=yes
; Installs for the current user without asking for administrator rights.
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
SetupIconFile=..\icons\webby.ico
UninstallDisplayIcon={app}\webby.ico
OutputDir=..\..\dist-installer
OutputBaseFilename=Webby-{#Version}-windows-x64-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\*"; DestDir: "{app}"; Flags: recursesubdirs ignoreversion
Source: "..\icons\webby.ico"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{group}\Webby"; Filename: "{app}\webby.exe"; IconFilename: "{app}\webby.ico"
Name: "{autodesktop}\Webby"; Filename: "{app}\webby.exe"; IconFilename: "{app}\webby.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\webby.exe"; Description: "{cm:LaunchProgram,Webby}"; Flags: nowait postinstall skipifsilent
