# sshc inline mode round-trip: pick → ssh (faked) → back to shell → reconnect.

Output docs/demos/inline-flow.gif

Set FontSize 14
Set Width 1000
Set Height 600
Set Theme "Catppuccin Mocha"
Set Shell "bash"

Hide
Type "export HOME=/tmp/sshc-demo-home"
Enter
Type "export PATH=/tmp/sshc-demo-home/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
Enter
Type "export PS1='$ '"
Enter
Type "clear"
Enter
Sleep 200ms
Show

Type "sshc"
Sleep 300ms
Enter
Sleep 1500ms

Type "prod"
Sleep 1500ms

Down
Sleep 400ms

Enter
Sleep 2500ms

Type "sshc"
Sleep 300ms
Enter
Sleep 1000ms

Type "r"
Sleep 2500ms

Sleep 500ms
