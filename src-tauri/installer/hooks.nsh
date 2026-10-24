; Current-user notification identity metadata only. Never change notification
; permission, policy, sound preferences, or register an unimplemented activator.
!macro NSIS_HOOK_POSTINSTALL
  WriteRegStr HKCU "Software\Classes\AppUserModelId\${BUNDLEID}" "DisplayName" "${PRODUCTNAME}"
  WriteRegStr HKCU "Software\Classes\AppUserModelId\${BUNDLEID}" "IconUri" "$INSTDIR\notification.png"
  WriteRegStr HKCU "Software\Classes\AppUserModelId\${BUNDLEID}" "WidgetInstallLocation" "$INSTDIR"
!macroend

!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ReadRegStr $R9 HKCU "Software\Classes\AppUserModelId\${BUNDLEID}" "WidgetInstallLocation"
    ${If} $R9 == "$INSTDIR"
      DeleteRegValue HKCU "Software\Classes\AppUserModelId\${BUNDLEID}" "DisplayName"
      DeleteRegValue HKCU "Software\Classes\AppUserModelId\${BUNDLEID}" "IconUri"
      DeleteRegValue HKCU "Software\Classes\AppUserModelId\${BUNDLEID}" "WidgetInstallLocation"
      DeleteRegKey /ifempty HKCU "Software\Classes\AppUserModelId\${BUNDLEID}"
    ${EndIf}
  ${EndIf}
!macroend
