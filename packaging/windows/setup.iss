; ==============================================================================
; Inno Setup Script for Uwu Log Viewer (Windows & WSL Client)
; ==============================================================================

#ifndef AppVersion
  #define AppVersion "0.1.0"
#endif

#ifndef AppArch
  #define AppArch "x64"
#endif

#ifndef SourceDir
  #define SourceDir "..\.."
#endif

#ifndef OutputDir
  #define OutputDir "..\..\dist"
#endif

#ifndef BinDir
  #define BinDir SourceDir + "\target\release"
#endif

[Setup]
AppId={{8B5C032F-E967-4224-B1F8-636402376C4F}
AppName=Uwu Log
AppVersion={#AppVersion}
AppVerName=Uwu Log {#AppVersion}
AppPublisher=gavuhop
AppPublisherURL=https://github.com/gavuhop/uwulog-rust
AppSupportURL=https://github.com/gavuhop/uwulog-rust/issues
AppUpdatesURL=https://github.com/gavuhop/uwulog-rust/releases
DefaultDirName={localappdata}\Programs\uwulog
DefaultGroupName=Uwu Log
AllowNoIcons=yes
OutputDir={#OutputDir}
#if AppArch == "arm64" || AppArch == "aarch64"
OutputBaseFilename=uwulog-setup-arm64
ArchitecturesAllowed=arm64
ArchitecturesInstallIn64BitMode=arm64
#else
OutputBaseFilename=uwulog-setup-x64
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
#endif
SetupIconFile={#SourceDir}\packaging\assets\icon.ico
UninstallDisplayIcon={app}\uwu-gui.exe
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
CloseApplications=yes
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
MinVersion=10.0.17763

#if GetEnv("WINDOWS_SIGN_CERT") != ""
SignTool=signtool sign /f "{#GetEnv('WINDOWS_SIGN_CERT')}" /p "{#GetEnv('WINDOWS_SIGN_PASS')}" /tr http://timestamp.digicert.com /td sha256 /fd sha256 $f
#endif

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "addtopath"; Description: "Add uwulog to Windows User PATH (Recommended for CMD, PowerShell & WSL)"; GroupDescription: "System Integration:"
Name: "desktopicon"; Description: "Create a Desktop shortcut"; GroupDescription: "Shortcuts:"

[Files]
; Main binaries
Source: "{#BinDir}\uwu-gui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BinDir}\uwu-tui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#BinDir}\uwu-agent.exe"; DestDir: "{app}"; Flags: ignoreversion
; CLI Launchers in bin\ (automatically inherited by WSL via Windows PATH)
Source: "{#BinDir}\uwulog.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#SourceDir}\packaging\windows\bin\uwulog"; DestDir: "{app}\bin"; Flags: ignoreversion
; Assets
Source: "{#SourceDir}\packaging\assets\icon.ico"; DestDir: "{app}"; Flags: ignoreversion
; Prebuilt Linux Agents for WSL provisioning (x86_64 & aarch64)
#if FileExists(SourceDir + "\agents\uwu-agent-x86_64")
Source: "{#SourceDir}\agents\uwu-agent-x86_64"; DestDir: "{app}\agents"; Flags: ignoreversion
#endif
#if FileExists(SourceDir + "\agents\uwu-agent-aarch64")
Source: "{#SourceDir}\agents\uwu-agent-aarch64"; DestDir: "{app}\agents"; Flags: ignoreversion
#endif
#if FileExists(BinDir + "\agents\uwu-agent-x86_64")
Source: "{#BinDir}\agents\uwu-agent-x86_64"; DestDir: "{app}\agents"; Flags: ignoreversion
#endif
#if FileExists(BinDir + "\agents\uwu-agent-aarch64")
Source: "{#BinDir}\agents\uwu-agent-aarch64"; DestDir: "{app}\agents"; Flags: ignoreversion
#endif
#if FileExists(SourceDir + "\target\x86_64-unknown-linux-musl\release\uwu-agent")
Source: "{#SourceDir}\target\x86_64-unknown-linux-musl\release\uwu-agent"; DestDir: "{app}\agents"; DestName: "uwu-agent-x86_64"; Flags: ignoreversion
#endif
#if FileExists(SourceDir + "\target\aarch64-unknown-linux-musl\release\uwu-agent")
Source: "{#SourceDir}\target\aarch64-unknown-linux-musl\release\uwu-agent"; DestDir: "{app}\agents"; DestName: "uwu-agent-aarch64"; Flags: ignoreversion
#endif

[Icons]
Name: "{group}\Uwu Log"; Filename: "{app}\uwu-gui.exe"; IconFilename: "{app}\icon.ico"
Name: "{group}\Uwu Log (TUI)"; Filename: "{app}\uwu-tui.exe"
Name: "{group}\Uninstall Uwu Log"; Filename: "{uninstallexe}"
Name: "{autodesktop}\Uwu Log"; Filename: "{app}\uwu-gui.exe"; IconFilename: "{app}\icon.ico"; Tasks: desktopicon

[Run]
Filename: "{app}\uwu-gui.exe"; Description: "Launch Uwu Log Viewer"; Flags: nowait postinstall skipifsilent

[Code]
const
  EnvironmentKey = 'Environment';

// Thêm bin vào User PATH
procedure AddToUserPath();
var
  OldPath, NewPath, BinDir: string;
begin
  BinDir := ExpandConstant('{app}\bin');
  if RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', OldPath) then
  begin
    if Pos(UpperCase(BinDir), UpperCase(OldPath)) = 0 then
    begin
      if (OldPath <> '') and (OldPath[Length(OldPath)] <> ';') then
        NewPath := OldPath + ';' + BinDir
      else
        NewPath := OldPath + BinDir;

      RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', NewPath);
    end;
  end
  else
  begin
    RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', BinDir);
  end;
end;

// Xoá bin khỏi User PATH khi Uninstall
procedure RemoveFromUserPath();
var
  OldPath, BinDir: string;
  P, L: Integer;
begin
  BinDir := ExpandConstant('{app}\bin');
  if RegQueryStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', OldPath) then
  begin
    P := Pos(UpperCase(BinDir), UpperCase(OldPath));
    if P > 0 then
    begin
      L := Length(BinDir);
      // Remove trailing semicolon if present
      if (P + L <= Length(OldPath)) and (OldPath[P + L] = ';') then
        L := L + 1
      // Or leading semicolon if at the end
      else if (P > 1) and (OldPath[P - 1] = ';') then
      begin
        P := P - 1;
        L := L + 1;
      end;

      Delete(OldPath, P, L);
      RegWriteStringValue(HKEY_CURRENT_USER, EnvironmentKey, 'Path', OldPath);
    end;
  end;
end;

procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssPostInstall then
  begin
    if WizardIsTaskSelected('addtopath') then
    begin
      AddToUserPath();
    end;
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RemoveFromUserPath();
  end;
end;
