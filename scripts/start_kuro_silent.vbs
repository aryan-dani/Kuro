Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

appData = WshShell.ExpandEnvironmentStrings("%LOCALAPPDATA%")
disableFlag = appData & "\Kuro\DISABLED"
If fso.FileExists(disableFlag) Then
  WScript.Quit 0
End If

scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
watchPs1 = scriptDir & "\Watch-KuroWifi.ps1"
If Not fso.FileExists(watchPs1) Then
  watchPs1 = scriptDir & "\Start-Kuro.ps1"
End If

' Prefer Wi-Fi watcher so Kuro only runs on home network
cmd = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File """ & watchPs1 & """"
WshShell.Run cmd, 0, False
