# Quit the CLI

`:q` ends the CLI proof. The process exits 0 and does not keep running.

## Sub-features

- `quit-exit` sends `:q` and requires exit code 0.
- `quit-no-save-required` quits without sending `:w`.

## How to get to it (user POV)

- At the `>` prompt, enter `:q`.

## Driving it with legion-app

Preconditions:

- Same workspace and binary as open-file.
- No other `legion-app` was started by this run.

- **Quit.** Run `target\debug\legion-app.exe hello.txt` with stdin `:q`. Stdout contains `Opened file id`. The process exits. Exit code is 0.
- **Proof.** Write combined output to `target/verify-legion-ide/evidence/<run-id>/quit.txt` and the exit code to `quit.exit`. `hello.txt` is still `seed` plus a newline.

## Gotchas

- Closing stdin (no `:q` line) also ends the read loop. That is not the user command. The recipe must send the characters `:q`.
- Do not stop a leftover process by image name. This recipe's process exits on its own.
