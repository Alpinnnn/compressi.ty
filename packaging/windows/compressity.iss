; ============================================================================
; Compressity Inno Setup installer script
; Branded to match the app's dark shell with static installer artwork
; ============================================================================

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
AppPublisherURL=https://github.com/Alpinnnn/compressi.ty
AppSupportURL=https://github.com/Alpinnnn/compressi.ty/issues
DefaultDirName={autopf}\Compressity
DefaultGroupName=Compressity
DisableProgramGroupPage=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist\windows\installer
OutputBaseFilename=Compressity-Setup-{#MyAppVersion}
Compression=lzma2/max
SolidCompression=yes
ChangesAssociations=yes

WizardStyle=modern dark
WizardSizePercent=120
WizardImageFile=
WizardSmallImageFile=..\..\assets\icon\icon.bmp
SetupIconFile=..\..\assets\icon\icon.ico
WizardSmallImageBackColor=none
WizardBackImageFile=installer-bg-welcome.png

ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
UninstallDisplayIcon={app}\compressity.exe
UninstallDisplayName={#MyAppName}

VersionInfoVersion={#MyAppVersion}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Additional shortcuts:"
Name: "shellintegration"; Description: "Add 'Open with Compressi.ty' to supported files"; GroupDescription: "Explorer integration:"

[Files]
Source: "installer-bg-welcome.png"; Flags: dontcopy noencryption
Source: "installer-bg-license.png"; Flags: dontcopy noencryption
Source: "installer-bg-select-dir.png"; Flags: dontcopy noencryption
Source: "installer-bg-select-tasks.png"; Flags: dontcopy noencryption
Source: "installer-bg-ready.png"; Flags: dontcopy noencryption
Source: "installer-bg-installing.png"; Flags: dontcopy noencryption
Source: "installer-bg-finished.png"; Flags: dontcopy noencryption
Source: "{#StageDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Registry]
Root: HKCR; Subkey: "SystemFileAssociations\.png\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.png\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.png\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.png\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpg\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpg\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpg\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpg\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpeg\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpeg\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpeg\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.jpeg\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webp\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webp\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webp\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webp\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avif\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avif\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avif\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avif\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mp4\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mp4\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mp4\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mp4\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mov\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mov\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mov\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mov\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mkv\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mkv\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mkv\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.mkv\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webm\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webm\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webm\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.webm\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avi\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avi\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avi\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.avi\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.m4v\shell\Compressity.Open"; ValueType: string; ValueName: ""; ValueData: "Open with Compressi.ty"; Flags: uninsdeletekey; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.m4v\shell\Compressity.Open"; ValueType: string; ValueName: "Icon"; ValueData: "{app}\compressity.exe,0"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.m4v\shell\Compressity.Open"; ValueType: string; ValueName: "MultiSelectModel"; ValueData: "Player"; Tasks: shellintegration
Root: HKCR; Subkey: "SystemFileAssociations\.m4v\shell\Compressity.Open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\compressity.exe"" ""%1"""; Flags: uninsdeletekey; Tasks: shellintegration

[Icons]
Name: "{autoprograms}\Compressity"; Filename: "{app}\compressity.exe"
Name: "{autodesktop}\Compressity"; Filename: "{app}\compressity.exe"; Tasks: desktopicon

[Run]
Filename: "{app}\compressity.exe"; Description: "Launch Compressity"; Flags: nowait postinstall skipifsilent

[Messages]
WelcomeLabel1=Welcome to Compressity
WelcomeLabel2=Setup will install [name/ver] on your computer.%n%nCompressity is a local-first desktop compression toolkit for photos and videos.%n%nClick Next to continue.
FinishedHeadingLabel=Installation Complete
FinishedLabel=Compressity has been installed on your computer.%n%nClick Finish to close Setup.

[Code]
// Apply the brand accent to the progress bar at runtime.
// These match the ThemeColors struct in theme.rs as closely as Inno Setup
// allows (the dark style already provides the base; we tweak on top of it).

const
  BackgroundWelcomeFile = 'installer-bg-welcome.png';
  BackgroundLicenseFile = 'installer-bg-license.png';
  BackgroundSelectDirFile = 'installer-bg-select-dir.png';
  BackgroundSelectTasksFile = 'installer-bg-select-tasks.png';
  BackgroundReadyFile = 'installer-bg-ready.png';
  BackgroundInstallingFile = 'installer-bg-installing.png';
  BackgroundFinishedFile = 'installer-bg-finished.png';
  PBM_SETBARCOLOR = $0409;

var
  BackgroundWelcomePath: String;
  BackgroundLicensePath: String;
  BackgroundSelectDirPath: String;
  BackgroundSelectTasksPath: String;
  BackgroundReadyPath: String;
  BackgroundInstallingPath: String;
  BackgroundFinishedPath: String;
  ActiveBackgroundPath: String;

function SendMessage(hWnd: HWND; Msg: UINT; wParam: Longint; lParam: Longint): Longint;
  external 'SendMessageW@user32.dll stdcall';

function BuildTempAssetPath(const FileName: String): String;
begin
  Result := ExpandConstant('{tmp}\') + FileName;
end;

function ResolveBackgroundPath(const CurPageID: Integer): String;
begin
  if CurPageID = wpWelcome then
    Result := BackgroundWelcomePath
  else if CurPageID = wpLicense then
    Result := BackgroundLicensePath
  else if CurPageID = wpSelectDir then
    Result := BackgroundSelectDirPath
  else if CurPageID = wpSelectTasks then
    Result := BackgroundSelectTasksPath
  else if CurPageID = wpReady then
    Result := BackgroundReadyPath
  else if (CurPageID = wpPreparing) or (CurPageID = wpInstalling) then
    Result := BackgroundInstallingPath
  else if CurPageID = wpFinished then
    Result := BackgroundFinishedPath
  else
    Result := BackgroundWelcomePath;
end;

procedure ApplyWizardBackground(const ImagePath: String);
var
  BackImages: TArrayOfGraphic;
begin
  if (ImagePath = '') or (ActiveBackgroundPath = ImagePath) then
    exit;

  SetLength(BackImages, 1);
  BackImages[0] := TPngImage.Create;
  try
    BackImages[0].LoadFromFile(ImagePath);
    WizardSetBackImage(BackImages, True, True, 255);
    ActiveBackgroundPath := ImagePath;
  finally
    BackImages[0].Free;
  end;
end;

procedure UpdateWizardBackground(const CurPageID: Integer);
var
  ImagePath: String;
begin
  try
    ImagePath := ResolveBackgroundPath(CurPageID);
    ApplyWizardBackground(ImagePath);
  except
    LogFmt('Could not switch wizard background on page %d: %s', [CurPageID, GetExceptionMessage]);
  end;
end;

procedure InitializeWizard();
begin
  SendMessage(WizardForm.ProgressGauge.Handle, PBM_SETBARCOLOR, 0, $003E8AFF);

  ExtractTemporaryFile(BackgroundWelcomeFile);
  ExtractTemporaryFile(BackgroundLicenseFile);
  ExtractTemporaryFile(BackgroundSelectDirFile);
  ExtractTemporaryFile(BackgroundSelectTasksFile);
  ExtractTemporaryFile(BackgroundReadyFile);
  ExtractTemporaryFile(BackgroundInstallingFile);
  ExtractTemporaryFile(BackgroundFinishedFile);

  BackgroundWelcomePath := BuildTempAssetPath(BackgroundWelcomeFile);
  BackgroundLicensePath := BuildTempAssetPath(BackgroundLicenseFile);
  BackgroundSelectDirPath := BuildTempAssetPath(BackgroundSelectDirFile);
  BackgroundSelectTasksPath := BuildTempAssetPath(BackgroundSelectTasksFile);
  BackgroundReadyPath := BuildTempAssetPath(BackgroundReadyFile);
  BackgroundInstallingPath := BuildTempAssetPath(BackgroundInstallingFile);
  BackgroundFinishedPath := BuildTempAssetPath(BackgroundFinishedFile);

  UpdateWizardBackground(WizardForm.CurPageID);
end;

procedure CurPageChanged(CurPageID: Integer);
begin
  UpdateWizardBackground(CurPageID);
end;
