# Runtime Isolation

git-paw isolates each agent's **source**: every agent gets its own git worktree,
so nobody stashes, nobody switches branches, and nobody overwrites a peer's
edits.

It does not, by default, isolate each agent's **runtime**. Sibling worktrees
read the same `.env` and bind the same ports, so the moment two agents start a
dev server, a preview, or a debugger, the second one fails with
`EADDRINUSE` — or worse, quietly talks to the first one's database.

Runtime isolation fixes the three collisions that matter: the shared env file,
the shared port, and — through a hook you write — the shared database. All are
opt-in, configured in `.git-paw/config.toml`:

```toml
[worktree.env]
copy = [".env", ".env.development"]

[worktree.ports]
base   = 3000
stride = 10
vars   = ["PORT", "VITE_PORT"]

[worktree.hooks]
on_create = "scripts/paw-db-branch.sh {worktree_id}"
on_remove = "scripts/paw-db-drop.sh {worktree_id}"
```

With no table present, nothing changes — no files are copied, no `.env.local`
is generated, no command runs, and worktrees are created exactly as they were
before. git-paw assumes no filename, no port, and no stack; you declare what
your project actually needs.

## Env files are copied, not linked

Each entry in `[worktree.env] copy` is copied verbatim from the repository root
into the new worktree, **after** the worktree is created and **before** the
agent's CLI process starts. The agent's very first read already sees a
provisioned checkout.

The copy is independent — never a symlink. That is the whole point: a symlinked
`.env` is shared mutable state across every agent, which is exactly the silent
runtime corruption worktrees exist to prevent. One agent flipping
`FEATURE_X=true` to debug its branch would flip it for everyone. A copy diverges
cleanly instead, and any drift you actually care about surfaces at merge time.

A declared file that does not exist at the repository root is skipped with a
warning and the worktree is still created. Per-branch divergence is normal, and
a missing optional env file should not cost you a session.

## Each worktree gets its own port block

Each worktree holds a **runtime slot**. Its port block starts at
`base + slot * stride`, and the Nth name in `vars` receives
`base + slot * stride + N`. With the configuration above:

| Worktree | Slot | `PORT` | `VITE_PORT` |
|---|---|---|---|
| first agent | 0 | 3000 | 3001 |
| second agent | 1 | 3010 | 3011 |
| third agent | 2 | 3020 | 3021 |

The assignments land in a delimited managed block in the worktree's
`.env.local`, the override layer most stacks load last:

```
# >>> git-paw (managed) >>>
PORT=3010
VITE_PORT=3011
# <<< git-paw (managed) <<<
```

Anything you keep in `.env.local` outside that block survives, and regenerating
the block is idempotent. The copied `.env` is never touched — offset ports live
only in `.env.local`, so `.env` stays a faithful copy of the source you can diff
against.

Keep `stride` at least as large as the number of `vars`, with headroom for vars
you add later. A smaller stride would overlap adjacent worktrees' blocks and
hand two live agents the same port; git-paw clamps the stride up and warns, but
it is better to get it right in config.

## Ports are deterministic, not probed

git-paw does not scan the host for free ports. A slot's block is computed from
config and the slot index, nothing else.

The trade is deliberate. Probing would dodge a busy port once, but the block a
worktree gets would change from run to run — so a bookmark, a proxy rule, or a
teammate's note about "the API is on 3010" would go stale every restart.
Deterministic assignment means a worktree keeps its ports for as long as it
exists.

The cost is that an assigned port can still be taken by something unrelated on
your machine. When that happens, change `base` or `stride` — you are picking a
range for the whole session, not fighting one process.

## Slots survive add/remove churn

The slot is recorded in session state rather than derived from the worktree's
position in the list. Position would be fragile: remove the *middle* worktree of
three and everything after it shifts down, so the next `git paw add` would be
handed a still-running agent's ports.

Instead, a new agent takes the lowest slot no live agent holds:

```console
$ git paw add feat/one     # slot 0 → 3000
$ git paw add feat/two     # slot 1 → 3010
$ git paw add feat/three   # slot 2 → 3020
$ git paw remove feat/two  # slot 1 is freed
$ git paw add feat/four    # slot 1 → 3010, reused
```

Blocks stay bounded instead of climbing forever, and two concurrently active
worktrees are never assigned overlapping blocks. `git paw purge` releases every
slot along with the session.

Session state written before runtime isolation existed loads with no slot and is
assigned one the next time the worktree is provisioned, so upgrading changes
nothing until you opt in.

## Giving each worktree its own database

Copying an env file and offsetting a port are things git-paw can do for any
project. Branching a **database** is not — it means a Neon API call on one
stack, `createdb` on another, a Docker volume on a third. So git-paw does not
ship a driver. It runs *your* command at the two moments that matter, and lets
that command hand its result back through `.env.local`.

Two scripts, one for each direction:

```sh
# scripts/paw-db-branch.sh — called with the worktree id
#!/bin/sh
set -e
createdb "paw_$1"
psql -q -d "paw_$1" -f db/schema.sql
echo "DATABASE_URL=postgres://localhost/paw_$1"
```

```sh
# scripts/paw-db-drop.sh — called with the same worktree id
#!/bin/sh
dropdb --if-exists "paw_$1"
```

Wire them up:

```toml
[worktree.hooks]
on_create = "scripts/paw-db-branch.sh {worktree_id}"
on_remove = "scripts/paw-db-drop.sh {worktree_id}"
```

Now `git paw start --branches feat/two` creates the worktree, copies the env
files, allocates the ports, then runs `paw-db-branch.sh feat-two` inside that
worktree. The one line the script printed to stdout joins the same managed
block:

```
# >>> git-paw (managed) >>>
PORT=3010
VITE_PORT=3011
DATABASE_URL=postgres://localhost/paw_feat-two
# <<< git-paw (managed) <<<
```

The agent's app reads `.env.local` and talks to its own database. A migration
one agent runs cannot corrupt another's fixtures.

`git paw remove feat/two` (or `git paw purge`) runs `paw-db-drop.sh feat-two`
in the worktree **before** deleting it, so the database goes away with the
branch.

Anything the hook prints that is not a `KEY=value` line is ignored, so your
script can log progress freely. Only the assignments become environment.

### Make teardown idempotent

Note the `--if-exists` in the drop script. `on_remove` is best-effort by
design: if it fails, git-paw warns and **removes the worktree anyway**, because
a failed teardown must never leave you with a stranded checkout. A crash or a
forced purge can skip the hook entirely.

That means your teardown script will sometimes run against a resource that is
already gone, and sometimes not run at all. Write it so a second run is
harmless, and reconcile leftovers out of band (a periodic sweep of `paw_*`
databases) rather than relying on the hook alone.

By contrast, a failing `on_create` **does** stop that worktree — git-paw
reports the hook's exit status and stderr rather than handing an agent a
half-built runtime.

> Print secrets on stdout and diagnostics on stderr. git-paw logs only the
> *names* of the variables it merged, so a password inside `DATABASE_URL` never
> reaches a log line — but a failing hook's stderr *is* shown. And since the
> value lands in a plain `.env.local`, make sure that file is gitignored.

> A hook is an arbitrary command from your config file, run with your
> privileges — the same trust model as a custom CLI's `command`. git-paw never
> builds one from anything an agent or the network supplied.

## Checking it worked

After starting a session, look inside any agent's worktree:

```console
$ cat ../myproject-feat-two/.env.local
# >>> git-paw (managed) >>>
PORT=3010
VITE_PORT=3011
# <<< git-paw (managed) <<<
```

If `.env.local` is missing, `[worktree.ports]` is absent or its `vars` list is
empty and no hook printed an assignment. If a declared env file is missing from
the worktree, check the warning `git paw start` printed — the file probably does
not exist at the repository root. If a hook key is missing, check that the
script prints it on **stdout** in exact `KEY=value` form.

## See also

- [Configuration](../configuration/README.md#worktree-runtime-provisioning) —
  the full `[worktree.env]` / `[worktree.ports]` / `[worktree.hooks]` key
  reference
- [Worktree Placement](worktree-placement.md) — where the worktrees themselves
  are created
