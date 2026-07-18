Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

root = fso.GetParentFolderName(WScript.ScriptFullName)
shell.CurrentDirectory = root

Set env = shell.Environment("PROCESS")
env("FORGE_ELECTRON_DESKTOP_STABLE") = "1"
env("FORGE_ELECTRON_DESKTOP_AUTO_REBUILD") = "0"
env("FORGE_ELECTRON_DESKTOP_DEV_SERVER") = "0"

' Keep the desktop shortcut silent. The canonical launcher can still always delegate freshness,
' rebuild, focus and restart decisions when run directly; the desktop shortcut uses stable fast path.
shell.Run """" & root & "\run_ingen_electron_shell.cmd" & """", 0, False
