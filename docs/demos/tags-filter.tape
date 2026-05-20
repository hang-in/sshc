# sshc manage mode tag filter: /@prod then /@dev.

Output docs/demos/tags-filter.gif

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

Type "sshc -m"
Sleep 300ms
Enter
Sleep 1500ms

Type "/"
Sleep 600ms

Type "@prod"
Sleep 2000ms

Backspace
Backspace
Backspace
Backspace
Backspace
Sleep 600ms

Type "@dev"
Sleep 2000ms

Escape
Sleep 500ms

Type "q"
Sleep 500ms
