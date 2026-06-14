Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

root = fso.GetParentFolderName(WScript.ScriptFullName)
repoRoot = fso.GetAbsolutePathName(root & "\..\..")
electronExe = root & "\node_modules\electron\dist\electron.exe"
mainBundle = root & "\dist-electron\main\main.js"
rendererIndex = root & "\dist\renderer\index.html"
backendExe = repoRoot & "\.codex-targets\ingen-electron-shortcut\debug\ingen_electron_backend_bridge.exe"
taskbarHelperExe = repoRoot & "\.codex-targets\ingen-electron-shortcut\debug\ingen_windows_taskbar_helper.exe"
legacyUserData = shell.ExpandEnvironmentStrings("%APPDATA%") & "\InGen"
runtimeUserData = shell.ExpandEnvironmentStrings("%APPDATA%") & "\InGenRuntime"

shell.CurrentDirectory = root

If shell.AppActivate("InGen") Then
  WScript.Quit 0
End If

forceRebuild = shell.ExpandEnvironmentStrings("%FORGE_ELECTRON_FORCE_REBUILD%")
autoRebuild = shell.ExpandEnvironmentStrings("%FORGE_ELECTRON_DESKTOP_AUTO_REBUILD%")
needsFullLauncher = forceRebuild = "1" Or autoRebuild = "1"
needsFullLauncher = needsFullLauncher Or Not fso.FileExists(electronExe)
needsFullLauncher = needsFullLauncher Or Not fso.FileExists(mainBundle)
needsFullLauncher = needsFullLauncher Or Not fso.FileExists(rendererIndex)
needsFullLauncher = needsFullLauncher Or Not fso.FileExists(backendExe)
needsFullLauncher = needsFullLauncher Or Not fso.FileExists(taskbarHelperExe)

If needsFullLauncher Then
  shell.Run """" & root & "\run_ingen_electron_shell.cmd" & """", 0, False
  WScript.Quit 0
End If

If Not fso.FolderExists(runtimeUserData) Then
  fso.CreateFolder(runtimeUserData)
End If

If Not fso.FolderExists(runtimeUserData & "\brain") Then
  If fso.FolderExists(legacyUserData & "\brain") Then
    fso.CopyFolder legacyUserData & "\brain", runtimeUserData & "\brain", True
  End If
End If

For Each fileName In Array("workspace.json", "llm-provider-runtime.json", "llm-providers.json", "llm-runtime-request.json", "native-session-ledger.json")
  If Not fso.FileExists(runtimeUserData & "\" & fileName) Then
    If fso.FileExists(legacyUserData & "\" & fileName) Then
      fso.CopyFile legacyUserData & "\" & fileName, runtimeUserData & "\" & fileName, True
    End If
  End If
Next

Set env = shell.Environment("PROCESS")
env("FORGE_FRONT_SLICE_HEADER") = "electron"
env("FORGE_FRONT_SLICE_SIDEBAR") = "electron"
env("FORGE_FRONT_SLICE_CANVAS") = "electron"
env("FORGE_FRONT_SLICE_RIGHT_PANEL") = "electron"
env("FORGE_FRONT_SLICE_PANELS_CHAT_BOTTOM") = "electron"
env("FORGE_CARGO_SESSION") = "ingen-electron-shortcut"
env("FORGE_ELECTRON_BACKEND_EXE") = backendExe
env("FORGE_WINDOWS_TASKBAR_HELPER_EXE") = taskbarHelperExe
env("INGEN_ELECTRON_USER_DATA_DIR") = runtimeUserData
env("INGEN_ELECTRON_DESKTOP_FAST_PATH") = "1"

shell.Run """" & electronExe & """ . ""--user-data-dir=" & runtimeUserData & """", 0, False
