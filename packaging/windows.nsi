Unicode true
!include "MUI2.nsh"
Name "FabLabCAM Studio ${VERSION} (${ARCH})"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\FabLabCAM"
RequestExecutionLevel user
SetCompressor /SOLID lzma
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "${SOURCE}\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "French"
!insertmacro MUI_LANGUAGE "English"
Section "FabLabCAM Studio"
  SetShellVarContext current
  SetOutPath "$INSTDIR"
  File /r "${SOURCE}\*"
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  CreateDirectory "$SMPROGRAMS\FabLabCAM"
  CreateShortCut "$SMPROGRAMS\FabLabCAM\FabLabCAM Studio.lnk" "$INSTDIR\fablabcam-studio-ui.exe"
  CreateShortCut "$SMPROGRAMS\FabLabCAM\Documentation.lnk" "https://fablabcam.github.io/docs/"
  CreateShortCut "$SMPROGRAMS\FabLabCAM\Désinstaller.lnk" "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FabLabCAM" "DisplayName" "FabLabCAM Studio"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FabLabCAM" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FabLabCAM" "Publisher" "FabLabCAM"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FabLabCAM" "UninstallString" '$"$INSTDIR\Uninstall.exe$"'
SectionEnd
Section "Uninstall"
  SetShellVarContext current
  Delete "$INSTDIR\fablabcam.exe"
  Delete "$INSTDIR\fablabcam-view.exe"
  Delete "$INSTDIR\fablabcam-studio-ui.exe"
  Delete "$INSTDIR\LICENSE"
  Delete "$INSTDIR\NOTICE"
  Delete "$INSTDIR\README.txt"
  Delete "$INSTDIR\profiles\mach3-mill.json"
  Delete "$INSTDIR\profiles\mach3-mill-demo.json"
  RMDir "$INSTDIR\profiles"
  Delete "$INSTDIR\THIRD-PARTY-LICENSES.html"
  RMDir /r "$INSTDIR\font-licenses"
  RMDir /r "$INSTDIR\third-party-sources"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  Delete "$SMPROGRAMS\FabLabCAM\FabLabCAM Studio.lnk"
  Delete "$SMPROGRAMS\FabLabCAM\Documentation.lnk"
  Delete "$SMPROGRAMS\FabLabCAM\Désinstaller.lnk"
  RMDir "$SMPROGRAMS\FabLabCAM"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\FabLabCAM"
SectionEnd
