# sshc manage mode probe column: workers fan out → glyphs flip from
# blank/inflight to ●/○.

Output docs/demos/probe-indicator.gif

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
Sleep 6000ms

Type "q"
Sleep 500ms
