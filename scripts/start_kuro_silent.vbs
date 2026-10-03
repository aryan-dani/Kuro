Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

appData = WshShell.ExpandEnvironmentStrings("%LOCALAPPDATA%")
disableFlag = appData & "\Kuro\DISABLED"
If fso.FileExists(disableFlag) Then
  WScript.Quit 0
End If

scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
startPs1 = scriptDir & "\Start-Kuro.ps1"

' Always go through Start-Kuro.ps1 so duplicates are collapsed / skipped
cmd = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File """ & startPs1 & """"
WshShell.Run cmd, 0, False
