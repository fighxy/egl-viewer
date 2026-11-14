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

; Photos and videos the app views on their own (D-180). Per user, under HKCU\Software\Classes:
; the app shows up in Explorer's "Open with" and in Settings > Default apps. Windows lets only
; the user make it the default, so nothing here takes a type over.
!define VIEWER_PROGID "egl-viewer-gpui.media"
!define VIEWER_CAPABILITIES "Software\${PRODUCT_DIR_NAME}\Capabilities"
!define VIEWER_APP_KEY "Software\Classes\Applications\${PRODUCT_EXE}"

; The same list as `file_viewer::extensions` in crates/gpui-desktop (a test keeps them equal).
!macro ForEachViewerExtension MACRO
  !insertmacro ${MACRO} "jpg"
  !insertmacro ${MACRO} "jpeg"
  !insertmacro ${MACRO} "jpe"
  !insertmacro ${MACRO} "jfif"
  !insertmacro ${MACRO} "png"
  !insertmacro ${MACRO} "gif"
  !insertmacro ${MACRO} "webp"
  !insertmacro ${MACRO} "tif"
  !insertmacro ${MACRO} "tiff"
  !insertmacro ${MACRO} "bmp"
  !insertmacro ${MACRO} "dib"
  !insertmacro ${MACRO} "svg"
  !insertmacro ${MACRO} "svgz"
  !insertmacro ${MACRO} "psd"
  !insertmacro ${MACRO} "heic"
  !insertmacro ${MACRO} "heif"
  !insertmacro ${MACRO} "hif"
  !insertmacro ${MACRO} "mp4"
  !insertmacro ${MACRO} "m4v"
  !insertmacro ${MACRO} "mov"
  !insertmacro ${MACRO} "webm"
  !insertmacro ${MACRO} "mkv"
  !insertmacro ${MACRO} "avi"
  !insertmacro ${MACRO} "wmv"
  !insertmacro ${MACRO} "mpg"
  !insertmacro ${MACRO} "mpeg"
  !insertmacro ${MACRO} "mts"
  !insertmacro ${MACRO} "m2ts"
  !insertmacro ${MACRO} "flv"
  !insertmacro ${MACRO} "f4v"
  !insertmacro ${MACRO} "3gp"
  !insertmacro ${MACRO} "ogv"
!macroend

!macro RegisterViewerExtension EXT
  WriteRegStr HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${VIEWER_PROGID}" ""
  WriteRegStr HKCU "${VIEWER_APP_KEY}\SupportedTypes" ".${EXT}" ""
  WriteRegStr HKCU "${VIEWER_CAPABILITIES}\FileAssociations" ".${EXT}" "${VIEWER_PROGID}"
!macroend

!macro UnregisterViewerExtension EXT
  DeleteRegValue HKCU "Software\Classes\.${EXT}\OpenWithProgids" "${VIEWER_PROGID}"
!macroend

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

  ; Opening a photo or video from Explorer runs `gpui-desktop.exe "<file>"` (D-180).
  WriteRegStr HKCU "Software\Classes\${VIEWER_PROGID}" "" "egl-viewer photo or video"
  WriteRegStr HKCU "Software\Classes\${VIEWER_PROGID}\DefaultIcon" "" "$INSTDIR\${PRODUCT_EXE},0"
  WriteRegStr HKCU "Software\Classes\${VIEWER_PROGID}\shell\open\command" "" \
    '"$INSTDIR\${PRODUCT_EXE}" "%1"'
  WriteRegStr HKCU "${VIEWER_APP_KEY}" "FriendlyAppName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "${VIEWER_APP_KEY}\shell\open\command" "" '"$INSTDIR\${PRODUCT_EXE}" "%1"'
  WriteRegStr HKCU "${VIEWER_CAPABILITIES}" "ApplicationName" "${PRODUCT_NAME}"
  WriteRegStr HKCU "${VIEWER_CAPABILITIES}" "ApplicationDescription" \
    "View photos and videos, and keep them in libraries."
  !insertmacro ForEachViewerExtension RegisterViewerExtension
  WriteRegStr HKCU "Software\RegisteredApplications" "${PRODUCT_NAME}" "${VIEWER_CAPABILITIES}"
  ; SHCNE_ASSOCCHANGED: Explorer picks the new handler up without a sign-out.
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"

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

  !insertmacro ForEachViewerExtension UnregisterViewerExtension
  DeleteRegKey HKCU "Software\Classes\${VIEWER_PROGID}"
  DeleteRegKey HKCU "${VIEWER_APP_KEY}"
  DeleteRegValue HKCU "Software\RegisteredApplications" "${PRODUCT_NAME}"
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0, p 0, p 0)"

  DeleteRegKey HKCU "${UNINSTALL_REG}"
  DeleteRegKey HKCU "Software\${PRODUCT_DIR_NAME}"
  ; App data under %LOCALAPPDATA%\egl-viewer-gpui is left alone on purpose.
SectionEnd
