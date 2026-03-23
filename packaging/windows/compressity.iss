#define MyAppName "Compressity"
#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif
#ifndef StageDir
  #define StageDir "..\..\dist\windows\Compressity"
#endif

[Setup]
AppId={{4B6D5E75-A4E2-47D2-AE2A-1CB29855E7BF}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher=Compressity
DefaultDirName={autopf}\Compressity
DefaultGroupName=Compressity
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist\windows\installer
OutputBaseFilename=Compressity-Setup-{#MyAppVersion}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\compressity.exe

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"

[Files]
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{autoprograms}\Compressity"; Filename: "{app}\compressity.exe"
Name: "{autodesktop}\Compressity"; Filename: "{app}\compressity.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\compressity.exe"; Description: "Launch Compressity"; Flags: nowait postinstall skipifsilent
