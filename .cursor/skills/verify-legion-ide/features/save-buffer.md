# Save the active buffer

`:w` saves the active buffer and prints a save line. The CLI cannot insert text, so the proved bytes are the bytes already in the open file.

## Sub-features

- `save-apply` sends `:w` and requires a `Saved file_id=` line.
- `save-bytes` reads `hello.txt` again and requires the same `seed` contents.

## How to get to it (user POV)

- At the `>` prompt, enter `:w`.
- Enter `:q` to leave.

## Driving it with legion-app

Preconditions:

- Same workspace and binary as open-file.
- `hello.txt` is `seed` plus a newline.

- **Save.** Run `target\debug\legion-app.exe hello.txt` with stdin lines `:w` then `:q`. Stdout contains `Saved file_id=` and a `snapshot=` field. Exit code is 0.
- **Confirm bytes.** Read `hello.txt` after the process exits. Contents are `seed` and a newline. A `Save did not apply:` line fails the proof.
- **Proof.** Write combined output to `target/verify-legion-ide/evidence/<run-id>/save.txt`, the exit code to `save.exit`, and the second file read to `save.hello.txt`.

## Gotchas

- `:w` does not type into the buffer. Changing the file from another process before save is a conflict path, not this recipe.
- A rejected save keeps the in-memory buffer dirty. The proof must include the on-disk read.
- Lines other than `:w` and `:q` are ignored and do not count as a save.
