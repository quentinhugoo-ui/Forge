Set shell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

root = fso.GetParentFolderName(WScript.ScriptFullName)
shell.CurrentDirectory = root

' Keep the desktop shortcut silent, but always delegate freshness,
' rebuild, focus and restart decisions to the canonical launcher.
shell.Run """" & root & "\run_ingen_electron_shell.cmd" & """", 0, False
