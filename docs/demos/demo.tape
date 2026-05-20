# Single combined demo: inline mode fuzzy filter -> ssh round-trip ->
# shell. Captures the v0.4.x fzf-style flow in ~12 seconds.

Output docs/demos/demo.gif

Set FontSize 14
Set Width 1000
Set Height 480
Set Theme "Catppuccin Mocha"
Set Shell "bash"
Set TypingSpeed 70ms

Hide
Type "export HOME=/tmp/sshc-demo-home"
Enter
Type "export PATH=/tmp/sshc-demo-home/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
Enter
Type "export PS1='$ '"
Enter
Type "clear"
Enter
Sleep 300ms
Show

Type "sshc"
Sleep 400ms
Enter
Sleep 1500ms

Type "prod"
Sleep 1500ms

Down
Sleep 500ms

Enter
Sleep 3000ms

Sleep 800ms
