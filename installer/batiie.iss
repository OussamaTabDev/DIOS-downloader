; Batiie — Windows installer (Inno Setup 6)
; Built automatically by BUILD.bat -> dist\Batiie-Setup.exe
;
; Installs for the current user (no admin needed) into
;   %LOCALAPPDATA%\Programs\Batiie
; so yt-dlp can update itself. Adds a Start Menu folder, an optional desktop
; shortcut, "start with Windows", and a normal uninstaller.

#define MyAppName "Batiie"
#define MyAppExe "Batiie.exe"
; BUILD.bat passes the version of Cargo.toml: /DMyAppVersion=1.0.1
; the files that go into the setup (gathered by BUILD.bat / the CI)
#ifndef Stage
  #define Stage "..\build-cache\stage"
#endif
#ifndef MyAppVersion
  #define MyAppVersion "1.0.0"
#endif

[Setup]
AppId={{69B6547C-3877-48CA-B530-BC38B2C40E24}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher=Batiie Team
DefaultDirName={localappdata}\Programs\Batiie
DefaultGroupName={#MyAppName}
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir=..\dist
OutputBaseFilename=Batiie-Setup
#ifndef NoIcon
SetupIconFile=..\crates\app\icon.ico
#endif
UninstallDisplayIcon={app}\{#MyAppExe}
VersionInfoVersion={#MyAppVersion}
VersionInfoCompany=Batiie
VersionInfoDescription=Batiie Setup
VersionInfoProductName={#MyAppName}
VersionInfoProductVersion={#MyAppVersion}
VersionInfoCopyright=Copyright (C) 2026 Batiie Team
AppPublisherURL=https://github.com/OussamaTabDev/DIOS-downloader
UninstallDisplayName={#MyAppName}
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "Create a desktop shortcut"; GroupDescription: "Shortcuts:"
Name: "autostart"; Description: "Start Batiie when Windows starts (recommended)"; GroupDescription: "Startup:"
Name: "torrentassoc"; Description: "Open .torrent files and magnet links with Batiie"; GroupDescription: "File associations:"

[Files]
Source: "{#Stage}\{#MyAppExe}"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#Stage}\THIRD-PARTY-NOTICES.txt"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
; the helpers go into {app}\tools; the app updates them there by itself
Source: "{#Stage}\tools\*"; DestDir: "{app}\tools"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#Stage}\extension\*"; DestDir: "{app}\extension"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#Stage}\extension-firefox\*"; DestDir: "{app}\extension-firefox"; Flags: ignoreversion recursesubdirs createallsubdirs
Source: "{#Stage}\batiie-firefox.xpi"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
; system runtime files, installed only when they are missing (see [Code])
#ifexist "..\build-cache\redist\vc_redist.x64.exe"
  #define HaveRedist
Source: "..\build-cache\redist\vc_redist.x64.exe"; DestDir: "{tmp}"; Flags: dontcopy
#endif

[InstallDelete]
; the program was called "DIOS Downloader" before: remove what the old name left
Type: files; Name: "{app}\DIOSDownloader.exe"
Type: files; Name: "{app}\dios-downloader-firefox.xpi"
Type: filesandordirs; Name: "{userprograms}\DIOS Downloader"
Type: files; Name: "{autodesktop}\DIOS Downloader.lnk"
; and "Batiaa" (1.1.0 - 1.4.0) before it
Type: files; Name: "{app}\Batiaa.exe"
Type: files; Name: "{app}\batiaa-firefox.xpi"
Type: filesandordirs; Name: "{userprograms}\Batiaa"
Type: files; Name: "{autodesktop}\Batiaa.lnk"
; older versions kept the helpers next to the program
Type: files; Name: "{app}\yt-dlp.exe"
Type: files; Name: "{app}\ffmpeg.exe"

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"
Name: "{group}\Browser extension folder"; Filename: "{app}\extension"
Name: "{group}\Uninstall {#MyAppName}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExe}"; Tasks: desktopicon

[Registry]
; the old name's entries (start with Windows, .torrent type, settings key) go away
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "DIOS Downloader"; Flags: deletevalue
Root: HKCU; Subkey: "Software\Classes\DIOSDownloader.torrent"; ValueType: none; Flags: deletekey
Root: HKCU; Subkey: "Software\DIOS"; ValueType: none; Flags: deletekey dontcreatekey
; and the ones "Batiaa" left (1.1.0 - 1.4.0)
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: none; ValueName: "Batiaa"; Flags: deletevalue
Root: HKCU; Subkey: "Software\Classes\Batiaa.torrent"; ValueType: none; Flags: deletekey
Root: HKCU; Subkey: "Software\Batiaa"; ValueType: none; Flags: deletekey dontcreatekey
; tells the app the installer already decided about "start with Windows"
Root: HKCU; Subkey: "Software\Batiie\Batiie"; ValueType: string; ValueName: "Installed"; ValueData: "1"; Flags: uninsdeletekey
; start in the background (next to the clock) when the user signs in
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Batiie"; ValueData: """{app}\{#MyAppExe}"" --background"; Flags: uninsdeletevalue; Tasks: autostart
Root: HKCU; Subkey: "Software\Classes\.torrent"; ValueType: string; ValueData: "Batiie.torrent"; Flags: uninsdeletevalue; Tasks: torrentassoc
Root: HKCU; Subkey: "Software\Classes\Batiie.torrent"; ValueType: string; ValueData: "BitTorrent file"; Flags: uninsdeletekey; Tasks: torrentassoc
Root: HKCU; Subkey: "Software\Classes\Batiie.torrent\shell\open\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Tasks: torrentassoc
Root: HKCU; Subkey: "Software\Classes\magnet"; ValueType: string; ValueData: "URL:Magnet Link"; Flags: uninsdeletekey; Tasks: torrentassoc
Root: HKCU; Subkey: "Software\Classes\magnet"; ValueType: string; ValueName: "URL Protocol"; ValueData: ""; Tasks: torrentassoc
Root: HKCU; Subkey: "Software\Classes\magnet\shell\open\command"; ValueType: string; ValueData: """{app}\{#MyAppExe}"" ""%1"""; Tasks: torrentassoc

[Run]
Filename: "{app}\{#MyAppExe}"; Description: "Open Batiie now"; Flags: nowait postinstall skipifsilent
; after an automatic update: start the new version again
Filename: "{app}\{#MyAppExe}"; Flags: nowait; Check: IsAutoUpdate


[UninstallRun]
Filename: "{cmd}"; Parameters: "/C taskkill /IM {#MyAppExe} /F"; Flags: runhidden; RunOnceId: "StopBatiie"

[UninstallDelete]
Type: filesandordirs; Name: "{app}\extension"
Type: filesandordirs; Name: "{app}\extension-firefox"
Type: filesandordirs; Name: "{app}\tools"

[Code]
function IsAutoUpdate: Boolean;
begin
  Result := ExpandConstant('{param:AUTOUPDATE|0}') = '1';
end;

{ the Visual C++ 2015-2022 runtime (vcruntime140.dll) }
function RuntimeMissing: Boolean;
var
  Installed: Cardinal;
begin
  Result := True;
  if RegQueryDWordValue(HKLM64, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64', 'Installed', Installed) and (Installed = 1)
     and FileExists(ExpandConstant('{sys}\vcruntime140.dll'))
     and FileExists(ExpandConstant('{sys}\vcruntime140_1.dll'))
     and FileExists(ExpandConstant('{sys}\msvcp140.dll')) then
    Result := False;
end;

procedure InstallRuntime;
var
  Code: Integer;
begin
#ifdef HaveRedist
  if not RuntimeMissing then exit;
  { an automatic update must not show any window }
  if WizardSilent then exit;
  try
    ExtractTemporaryFile('vc_redist.x64.exe');
  except
    exit;
  end;
  WizardForm.StatusLabel.Caption := 'Installing required system files...';
  { needs administrator rights: Windows asks once. If that is refused the
    program still works, it carries its own copy of these files. }
  ShellExec('runas', ExpandConstant('{tmp}\vc_redist.x64.exe'), '/install /quiet /norestart', '', SW_HIDE, ewWaitUntilTerminated, Code);
#endif
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
    InstallRuntime;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
var
  Code: Integer;
begin
  { the running program locks its files: close it (it saves everything and
    the downloads go on after the update) }
  Exec(ExpandConstant('{cmd}'), '/C taskkill /IM {#MyAppExe} >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Sleep(2000);
  Exec(ExpandConstant('{cmd}'), '/C taskkill /F /IM {#MyAppExe} >nul 2>&1', '', SW_HIDE, ewWaitUntilTerminated, Code);
  Sleep(500);
  Result := '';
end;

