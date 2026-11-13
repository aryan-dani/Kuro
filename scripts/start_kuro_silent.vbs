Set WshShell = CreateObject("WScript.Shell")
Set fso = CreateObject("Scripting.FileSystemObject")

scriptDir = fso.GetParentFolderName(WScript.ScriptFullName)
appData = WshShell.ExpandEnvironmentStrings("%LOCALAPPDATA%")
disableFlag = appData & "\Kuro\DISABLED"

Dim targetPs1, extraArgs, i
extraArgs = ""

If WScript.Arguments.Count > 0 Then
  targetPs1 = WScript.Arguments(0)
  If Not fso.FileExists(targetPs1) Then
    targetPs1 = scriptDir & "\" & targetPs1
  End If
  For i = 1 To WScript.Arguments.Count - 1
    extraArgs = extraArgs & " " & WScript.Arguments(i)
  Next
Else
  If fso.FileExists(disableFlag) Then
    WScript.Quit 0
  End If
  targetPs1 = scriptDir & "\Watch-Kuro.ps1"
  If Not fso.FileExists(targetPs1) Then
    targetPs1 = scriptDir & "\Start-Kuro.ps1"
  End If
End If

cmd = "powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File """ & targetPs1 & """ -Silent" & extraArgs
WshShell.Run cmd, 0, False
