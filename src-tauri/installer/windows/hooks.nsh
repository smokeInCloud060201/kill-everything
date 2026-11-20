!macro NSIS_HOOK_PREUNINSTALL
  ; Terminate running KillEverything instances gracefully/forcefully before deleting files
  nsExec::ExecToLog 'taskkill /F /IM kill-everything.exe'

  ; Remove Launch-at-Login registry entry
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "KillEverything"

  ; Purge SQLite DB, WAL journals, ephemeral run scripts, and WebView2 cache
  RMDir /r "$APPDATA\com.killeverything.app"
  RMDir /r "$LOCALAPPDATA\com.killeverything.app"
!macroend
