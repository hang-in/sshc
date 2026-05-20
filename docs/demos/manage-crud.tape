# sshc manage mode: open form, fill fields, submit, observe list update.

Output docs/demos/manage-crud.gif

Set FontSize 14
Set Width 1000
Set Height 600
Set Theme "Catppuccin Mocha"
Set Shell "bash"
Set TypingSpeed 80ms

Hide
Type "export HOME=/tmp/sshc-demo-home"
Enter
Type "export PATH=/tmp/sshc-demo-home/bin:/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin"
Enter
Type "export PS1='$ '"
Enter
Type "clear"
Enter
Sleep 400ms
Show

Type "sshc -m"
Sleep 600ms
Enter
Sleep 3000ms

Type "a"
Sleep 1500ms

Type "demo-server"
Sleep 700ms

Tab
Sleep 500ms

Type "demo.example.com"
Sleep 700ms

Tab
Sleep 400ms

Type "ubuntu"
Sleep 600ms

Tab
Sleep 400ms

Type "2200"
Sleep 600ms

Tab
Sleep 400ms

Tab
Sleep 500ms

Enter
Sleep 2500ms

Type "q"
Sleep 500ms
