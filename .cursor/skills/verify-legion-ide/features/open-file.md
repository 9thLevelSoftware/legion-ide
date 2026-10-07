# Open a workspace file

Opening a file starts the CLI proof in a trusted workspace and prints the opened file id plus the command list.

## Sub-features

- `open-existing` opens `hello.txt` when that path exists in the workspace.
- `open-shows-commands` prints `Commands: :w | :q` before the prompt.

## How to get to it (user POV)

- From a terminal whose current directory is the workspace, run `legion-app hello.txt`.

## Driving it with legion-app

Preconditions:

- `target\debug\legion-app.exe` exists (extensionless `legion-app` off Windows).
- Working directory is `target/verify-legion-ide/runs/<run-id>/workspace`.
- `hello.txt` exists and contains `seed` plus a newline.

- **Start.** Run `target\debug\legion-app.exe hello.txt` with stdin `:q` so the process exits. Stdout contains `Opened file id` and `Commands: :w | :q`. Exit code is 0.
- **Proof.** Save that combined output and the exit code under `target/verify-legion-ide/evidence/<run-id>/open.txt` and `open.exit`.

## Gotchas

- The workspace root is the process current directory, not the file argument. Running at the repository root trusts the whole checkout.
- A missing file when a path was passed aborts startup. The scratch-file fallback applies only when no path argument is given.
- `Opened file id` without a later quit still leaves the process waiting on stdin.
