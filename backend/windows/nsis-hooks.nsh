!include LogicLib.nsh
!include nsDialogs.nsh

Var ChaWorkRootDir

!macro CHAWORK_RESOLVE_DEFAULT_ROOT
  ReadEnvStr $ChaWorkRootDir "CHAWORK_ROOT_DIR"
  ${If} $ChaWorkRootDir != ""
    Goto chawork_resolved_root
  ${EndIf}
  ReadRegStr $ChaWorkRootDir HKCU "Software\ChaWork" "RootDir"
  ${If} $ChaWorkRootDir == ""
    StrCpy $ChaWorkRootDir "$APPDATA\com.chawork.app\root"
  ${EndIf}
chawork_resolved_root:
!macroend

!macro CHAWORK_VALIDATE_ROOT_DIR
  ${If} ${FileExists} "$ChaWorkRootDir\.chawork-root"
    Goto chawork_root_valid
  ${EndIf}
  ${If} ${FileExists} "$ChaWorkRootDir\state\*.*"
  ${AndIf} ${FileExists} "$ChaWorkRootDir\employees\*.*"
  ${AndIf} ${FileExists} "$ChaWorkRootDir\runtime\*.*"
    Goto chawork_root_valid
  ${EndIf}
  ${If} ${FileExists} "$ChaWorkRootDir\*.*"
    MessageBox MB_ICONSTOP \
      "The selected ChaWork root workspace folder is not empty and is not an existing ChaWork root:$\r$\n$ChaWorkRootDir$\r$\n$\r$\nChoose an empty folder or an existing ChaWork root workspace."
    Abort
  ${EndIf}
chawork_root_valid:
!macroend

!macro CHAWORK_WRITE_ROOT_CONFIG
  CreateDirectory "$APPDATA\com.chawork.app"
  ; Persist the root workspace path in the registry (natively UTF-16LE ― no
  ; encoding ambiguity).  The app reads HKCU\Software\ChaWork\RootDir at
  ; startup.  Writing root-dir.txt is intentionally removed to avoid the
  ; ANSI/UTF-8 mismatch that breaks non-ASCII usernames (e.g. Chinese).
  WriteRegStr HKCU "Software\ChaWork" "RootDir" "$ChaWorkRootDir"
!macroend

!macro CHAWORK_REMOVE_MARKED_ROOT ROOT_DIR
  ${If} "${ROOT_DIR}" != ""
  ${AndIf} ${FileExists} "${ROOT_DIR}\.chawork-root"
    RMDir /r "${ROOT_DIR}"
  ${EndIf}
!macroend

!macro NSIS_HOOK_PREINSTALL
  !insertmacro CHAWORK_RESOLVE_DEFAULT_ROOT
  ${IfNot} ${Silent}
    MessageBox MB_YESNO|MB_ICONQUESTION \
      "ChaWork stores its root workspace in:$\r$\n$ChaWorkRootDir$\r$\n$\r$\nChoose a different root workspace folder?" \
      IDNO chawork_root_done
    nsDialogs::SelectFolderDialog "Select ChaWork root workspace folder" "$ChaWorkRootDir"
    Pop $0
    ${If} $0 != "error"
    ${AndIf} $0 != ""
      StrCpy $ChaWorkRootDir $0
    ${EndIf}
  ${EndIf}
chawork_root_done:
  !insertmacro CHAWORK_VALIDATE_ROOT_DIR
  ClearErrors
  CreateDirectory "$ChaWorkRootDir"
  IfErrors 0 chawork_root_created
    MessageBox MB_ICONSTOP "Failed to create ChaWork root workspace folder:$\r$\n$ChaWorkRootDir"
    Abort
chawork_root_created:
!macroend

!macro NSIS_HOOK_POSTINSTALL
  !insertmacro CHAWORK_WRITE_ROOT_CONFIG
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ReadRegStr $ChaWorkRootDir HKCU "Software\ChaWork" "RootDir"
  !insertmacro CHAWORK_REMOVE_MARKED_ROOT "$ChaWorkRootDir"
  !insertmacro CHAWORK_REMOVE_MARKED_ROOT "$APPDATA\com.chawork.app\root"
  RMDir /r "$APPDATA\com.chawork.app"
  DeleteRegKey HKCU "Software\ChaWork"
!macroend
