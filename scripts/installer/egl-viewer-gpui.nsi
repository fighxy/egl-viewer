; Per-user NSIS installer for the GPUI desktop host.
; Distinct from the Tauri product (egl-viewer / com.fighxy.eglviewer):
;   install dir  %LOCALAPPDATA%\Programs\egl-viewer-gpui
;   display name "egl-viewer (GPUI)"
; Does not touch %LOCALAPPDATA%\egl-viewer-gpui (app data).
; Authenticode signing is deferred — leave the built .exe unsigned.
;
; Build (after staging the portable layout):
;   makensis /DPRODUCT_VERSION=0.2.77 ^
;            /DSTAGE_DIR=dist\gpui-desktop ^
;            /DOUT_FILE=dist\egl-viewer-gpui_0.2.77_windows-x64-setup.exe ^
;            scripts\installer\egl-viewer-gpui.nsi

!ifndef PRODUCT_VERSION
  !error "Pass /DPRODUCT_VERSION=X.Y.Z"
!endif
!ifndef STAGE_DIR
  !error "Pass /DSTAGE_DIR=path\to\portable\stage"
!endif
!ifndef OUT_FILE
  !error "Pass /DOUT_FILE=path\to\setup.exe"
!endif

!define PRODUCT_NAME "egl-viewer (GPUI)"
!define PRODUCT_DIR_NAME "egl-viewer-gpui"
!define PRODUCT_EXE "gpui-desktop.exe"
!define PRODUCT_PUBLISHER "egl-viewer"
!define UNINSTALL_REG "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_DIR_NAME}"

Unicode true
ManifestDPIAware true
RequestExecutionLevel user
Name "${PRODUCT_NAME}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\Programs\${PRODUCT_DIR_NAME}"
InstallDirRegKey HKCU "Software\${PRODUCT_DIR_NAME}" "InstallDir"
ShowInstDetails show
ShowUninstDetails show
SetCompressor /SOLID lzma

; Silent installs (`/S`) and `/D=path` work with the NSIS defaults.
; Do not invent an MSI product code.

Page directory
Page instfiles
UninstPage uninstConfirm
UninstPage instfiles

Section "Install"
  SetOutPath "$INSTDIR"

  ; Same payload as the portable zip: exe, heif/libde265 next to it, notices.
  File /r "${STAGE_DIR}\*.*"

  WriteRegStr HKCU "Software\${PRODUCT_DIR_NAME}" "InstallDir" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_REG}" "DisplayName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "${UNINSTALL_REG}" "DisplayVersion" "${PRODUCT_VERSION}"
  WriteRegStr HKCU "${UNINSTALL_REG}" "Publisher" "${PRODUCT_PUBLISHER}"
  WriteRegStr HKCU "${UNINSTALL_REG}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${UNINSTALL_REG}" "DisplayIcon" "$INSTDIR\${PRODUCT_EXE}"
  WriteRegStr HKCU "${UNINSTALL_REG}" "UninstallString" '"$INSTDIR\Uninstall.exe"'
  WriteRegStr HKCU "${UNINSTALL_REG}" "QuietUninstallString" '"$INSTDIR\Uninstall.exe" /S'
  WriteRegDWORD HKCU "${UNINSTALL_REG}" "NoModify" 1
  WriteRegDWORD HKCU "${UNINSTALL_REG}" "NoRepair" 1

  WriteUninstaller "$INSTDIR\Uninstall.exe"

  CreateDirectory "$SMPROGRAMS\${PRODUCT_NAME}"
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk" \
    "$INSTDIR\${PRODUCT_EXE}" "" "$INSTDIR\${PRODUCT_EXE}" 0
  CreateShortCut "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall.lnk" \
    "$INSTDIR\Uninstall.exe"
  ; Current-user desktop only (`RequestExecutionLevel user` → $DESKTOP is not all-users).
  CreateShortCut "$DESKTOP\${PRODUCT_NAME}.lnk" \
    "$INSTDIR\${PRODUCT_EXE}" "" "$INSTDIR\${PRODUCT_EXE}" 0
SectionEnd

Section "Uninstall"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir /r "$INSTDIR"

  Delete "$SMPROGRAMS\${PRODUCT_NAME}\${PRODUCT_NAME}.lnk"
  Delete "$SMPROGRAMS\${PRODUCT_NAME}\Uninstall.lnk"
  RMDir "$SMPROGRAMS\${PRODUCT_NAME}"
  Delete "$DESKTOP\${PRODUCT_NAME}.lnk"

  DeleteRegKey HKCU "${UNINSTALL_REG}"
  DeleteRegKey HKCU "Software\${PRODUCT_DIR_NAME}"
  ; App data under %LOCALAPPDATA%\egl-viewer-gpui is left alone on purpose.
SectionEnd
