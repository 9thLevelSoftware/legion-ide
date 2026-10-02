export const meta = {
  name: 'legion-completion-round',
  description: 'One bounded packet round for Legion IDE full-product completion: plan, implement, serialized cargo check, independent review, serialized cargo test, adversarial refutation, record and commit',
  phases: [
    { title: 'State', detail: 'git/ledger/register snapshot and preconditions' },
    { title: 'Plan', detail: 'select disjoint packets and write briefs' },
    { title: 'Implement', detail: 'parallel implementers, owned files only, no cargo' },
    { title: 'Check', detail: 'single cargo lane: cargo check plus bounded compile-fix loop' },
    { title: 'Test', detail: 'single cargo lane: per-packet named tests, with one fix attempt' },
    { title: 'Review', detail: 'independent reviewer per packet, reading the test log, up to two blocking-fix passes' },
    { title: 'Gates', detail: 'single cargo lane: preserve and revert rejects, regression guard, clippy, fast gates' },
    { title: 'Refute', detail: 'adversarial verification of every claimed pass from raw logs' },
    { title: 'Record', detail: 'register update, ledger, evidence, split commits, push' },
  ],
}

const round = args.round
const stamp = args.stamp
const mode = args.mode || 'packets'
const nativeGuiResumed = args.nativeGuiResumed === true
const pushAuthorized = args.pushAuthorized === true
const focus = args.focus || ''

const WT = 'D:/legion-ide-completion'
const LEDGER = WT + '/.superpowers/sdd/2026-09-04-full-product-completion'
const EVID = WT + '/plans/evidence/full-product-resume-2026-09-08'
const NODE = 'C:/Users/dasbl/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/bin/node.exe'
const CHOKE = [
  'crates/legion-app/src/lib.rs',
  'crates/legion-ui/src/ui.rs',
  'crates/legion-desktop/src/view.rs',
  'crates/legion-protocol/src/lib.rs',
]

const RULES = [
  'Workspace is ' + WT + ' on branch codex/full-product-resume. Never read, write, or run anything in D:/legion-ide: that is the preserved planning worktree.',
  'Round ' + round + '. Local ledger (gitignored): ' + LEDGER + '.',
  'CARGO LANE RULE: you may not run cargo, rustc, or xtask unless your role is exactly "cargo lane". Before any such command check that ' + LEDGER + '/cargo.lock-owner does not exist and that no cargo.exe or rustc.exe is running; if either is busy, stop and report blocked instead of waiting.',
  'Only the role "record" may run git commit, git push, or write-mode gh. Only the role "record" may edit plans/completion/requirements.json.',
  'READ-ONLY gh IS OPEN TO EVERY ROLE and you are expected to use it: gh run list, gh run view, gh pr view, gh pr checks. This workspace runs its real three-OS gates only on the hosted runners, so a local Windows pass is not evidence that the branch is green. Nine consecutive hosted gate failures went unnoticed for eight rounds because no role read them. If you are the planner, read the hosted gate outcome for the current head before choosing packets and treat a red gate as the highest-priority work. If you are a reviewer, check whether the packet you are reviewing could plausibly break a platform this host cannot run.',
  'A register-lane packet MAY author the missing canonical register files plans/completion/matrix.json, scenarios.json, dependencies.json and defects.json, and may add an owner-approval section to plans/completion/decisions.md. Only requirements.json and candidate.json are reserved: requirements.json to the record role, candidate.json to a later owner-authorised nomination step.',
  'acceptance in requirements.json stays unassessed unless a validated EvidenceRun exists; a reviewer or implementer may never change it.',
  'No fabricated proposals, fixtures, exemptions, approvals, or evidence. Component evidence is reported as component evidence. An unavailable host, tool, or credential is recorded as blocked with an exact prerequisite string, never as passed or skipped.',
  'Native GUI automation is ' + (nativeGuiResumed ? 'RESUMED on this Windows host by owner instruction; macOS and Linux hosts remain unavailable and their rows stay blocked.' : 'PAUSED by the owner; do not launch a window.'),
  'Register ratification is provisional: owner_approval_ref values point at a dated entry in plans/completion/decisions.md that states the ratification is provisional and agent-made. Release mode must reject provisional cells.',
  'Chokepoint files (' + CHOKE.join(', ') + ') may not grow against origin/main; extract the touched region in a pure move first.',
].join('\n')

const S = function (props, required) { return { type: 'object', properties: props, required: required } }
const str = { type: 'string' }
const arr = { type: 'array', items: { type: 'string' } }
const bool = { type: 'boolean' }
const num = { type: 'number' }

const PACKET = S({
  packet_id: str, package_id: str, title: str, requirement_ids: arr,
  role: { type: 'string', enum: ['terra', 'sol', 'luna'] },
  owned_files: arr, new_files: arr, crates: arr, named_tests: arr,
  needs_real_server: bool, needs_native_gui: bool, brief: str, goal: str,
}, ['packet_id', 'package_id', 'title', 'requirement_ids', 'role', 'owned_files', 'new_files', 'crates', 'named_tests', 'needs_real_server', 'needs_native_gui', 'brief', 'goal'])

const STATE = S({
  head: str, clean: bool, dirty: arr, stray_cargo: bool, stale_lock: bool,
  disk_free_gb: num, in_flight: arr, next_work: arr, summary: str,
}, ['head', 'clean', 'dirty', 'stray_cargo', 'stale_lock', 'disk_free_gb', 'in_flight', 'next_work', 'summary'])

const PLAN = S({ packets: { type: 'array', items: PACKET }, owner_blocked: arr, escalations: arr, rationale: str },
  ['packets', 'owner_blocked', 'escalations', 'rationale'])

const IMPL = S({ status: { type: 'string', enum: ['done', 'partial', 'blocked'] }, files_changed: arr, report: str, claims: arr, untested_claims: arr },
  ['status', 'files_changed', 'report', 'claims', 'untested_claims'])

const STEP = S({ kind: str, packet_id: str, cmd: str, log: str, exit_code: { type: 'integer' }, claimed_pass: bool, summary: str, diagnostics: str },
  ['kind', 'packet_id', 'cmd', 'log', 'exit_code', 'claimed_pass', 'summary', 'diagnostics'])

const LANE = S({ steps: { type: 'array', items: STEP }, failed_packets: arr, notes: str }, ['steps', 'failed_packets', 'notes'])

const REVIEW = S({
  verdict: { type: 'string', enum: ['approve', 'changes', 'reject'] },
  blocking_findings: arr, nonblocking_findings: arr,
  proposed_implementation: { type: 'object', additionalProperties: str }, review_path: str, rationale: str,
}, ['verdict', 'blocking_findings', 'nonblocking_findings', 'proposed_implementation', 'review_path', 'rationale'])

const VERDICT = S({ refuted: bool, reason: str }, ['refuted', 'reason'])

const RECORD = S({ commits: arr, pushed: bool, pr_url: str, register_delta: str, escalations: arr, notes: str },
  ['commits', 'pushed', 'pr_url', 'register_delta', 'escalations', 'notes'])

const own = function (p) { return p.owned_files.concat(p.new_files) }

const lane = function (kind, body, ph) {
  return agent(RULES + '\n\nRole: cargo lane. You are the ONLY cargo owner in this round.\n' +
    'Before the first command write ' + LEDGER + '/cargo.lock-owner recording owner cargo-lane, round ' + round + ', kind ' + kind + '; update its step field before each command; delete it after the last command even on failure.\n' +
    'Working directory ' + WT + '. Every cargo command takes -j 1. Run exactly one command at a time; never run two in parallel.\n' +
    'Export LEGION_TEST_NODE_RUNTIME=' + NODE + ' in every shell you use.\n' +
    'Every step gets its own log at ' + LEDGER + '/round-' + round + '-' + kind + '-<step>.log with this exact provenance shape, because a later agent must be able to verify the step ran without trusting you: first a header line CMD: <the exact command line you ran>, then a line CWD: <working directory>, then a line BEGIN, then the captured stdout AND stderr verbatim, then a line END, then EXIT=<code>. A command that succeeds silently still produces CMD/CWD/BEGIN/END/EXIT with an empty body, which is what proves it ran. Never hand-write, summarise, or paraphrase output into a log; capture it.\n' +
    'Do not invent a step for work that had nothing to do. If a step list item is vacuous this round, omit the step and say so in notes rather than writing a narration-only log.\n' +
    'Commands that may exceed ten minutes must be started with run_in_background and polled; do not let a tool call time out and then assume a result.\n' +
    'Never edit source files. Never summarise a pass you did not read in the log. Report exact counts as logged.\n\n' + body,
    { label: 'cargo lane ' + kind, phase: ph, schema: LANE })
}

phase('State')
const state = await agent(RULES + '\n\nRole: state reader. Read-only; run no cargo and change nothing.\n' +
  'Report: git HEAD and whether the tree is clean (list dirty paths); whether cargo.exe or rustc.exe is running; whether ' + LEDGER + '/cargo.lock-owner exists; free GB on drive D; packets from ' + LEDGER + '/packets.json whose state is not recorded or reverted; the blockers already listed in ' + LEDGER + '/blockers.json; a short summary of the last 40 lines of ' + LEDGER + '/progress.md; and the "Next concrete work after resume" list from ' + WT + '/docs/superpowers/handoffs/2026-09-08-full-product-resume-handoff.md as next_work.',
  { label: 'state', phase: 'State', schema: STATE, effort: 'low' })

if (!state) return { round: round, halt: 'state agent died' }
const expectedDirty = args.expectedDirty || []
const unexplained = (state.dirty || []).filter(function (d) {
  return !expectedDirty.some(function (e) { return d.indexOf(e) >= 0 })
})
if (!state.clean && unexplained.length) return { round: round, halt: 'dirty tree', dirty: state.dirty, unexplained: unexplained }
if (!state.clean) log('carry-over work on disk, expected: ' + state.dirty.join(', '))
if (state.stray_cargo) return { round: round, halt: 'stray cargo or rustc process' }
if (state.stale_lock) return { round: round, halt: 'stale cargo lock owner file', hint: LEDGER + '/cargo.lock-owner' }
if (state.disk_free_gb < 20) return { round: round, halt: 'low disk', disk_free_gb: state.disk_free_gb }

if (mode === 'full-suite') {
  const full = await lane('full-suite',
    'Single step: cargo test --workspace --all-targets -j 1 --no-fail-fast, started with run_in_background and polled to completion.\n' +
    'This run was interrupted during compilation at the previous checkpoint and has no recorded outcome. Report per-target pass and fail counts exactly as the log shows. A build that did not finish is not a result; say so plainly. Set claimed_pass only if the log shows every target ran and none failed. Put the full list of failing test names in diagnostics.',
    'Test')
  if (!full || !full.steps.length) return { round: round, halt: 'cargo lane died', phase: 'full-suite' }
  const fs0 = full.steps[0]
  const fv = await agent('Try to refute this claimed workspace test result using ONLY the raw log at ' + fs0.log + '. Check that the log shows test binaries running (not only Compiling lines), that a "test result:" line exists for each target, that no line says the run was interrupted, and that the trailing EXIT marker matches. Default refuted=true when uncertain. Claim: ' + JSON.stringify(fs0),
    { label: 'refute full-suite', phase: 'Refute', schema: VERDICT, effort: 'high' })
  const triage = await agent(RULES + '\n\nRole: record. Append a progress.md entry for round ' + round + ' describing the workspace suite outcome exactly as logged, ending with a "Ruling: ... Cost if wrong: ..." line and the standing line that the goal remains unfinished, not blocked or complete. Copy the raw log to ' + EVID + '/ (create the directory and a README.md that classifies it as a workspace suite result, not product acceptance). Update packets.json with this round. Commit only the evidence directory with subject "[record ' + round + '] Record workspace suite outcome". ' + (pushAuthorized ? 'Then push to origin/codex/full-product-resume.' : 'Do not push.') + ' Stamp ' + stamp + '. Result to record: ' + JSON.stringify(fs0) + '. Refutation verdict: ' + JSON.stringify(fv),
    { label: 'record full-suite', phase: 'Record', schema: RECORD })
  return { round: round, mode: mode, stamp: stamp, result: fs0, refuted: !fv || fv.refuted, failing: fs0.diagnostics, record: triage }
}

phase('Plan')
const maxPackets = budget.total ? Math.max(2, Math.min(6, Math.floor(budget.remaining() / 300000))) : 4
const plan = await agent(RULES + '\n\nRole: planner. Read-only apart from writing brief files. Run no cargo.\n' +
  'Select up to ' + maxPackets + ' packets that can be implemented in parallel this round.\n' +
  'Priority order: the handoff next-work list first (bounded process stdin, then Python formatter provisioning and settings, then the pre-candidate register validation path), then the canonical register files, then remaining S2 language work, then S1 breadth, then S3 to S5, then the XQ qualification track. Include at most one register-lane packet per round.\n' +
  'Otherwise choose the lowest stage with required rows whose implementation is absent, partial, or repair-required and whose depends_on rows are all implemented, from ' + WT + '/plans/completion/requirements.json.\n' +
  'Hard constraints: no two packets may own the same file; at most one packet may touch a chokepoint file and it must shrink it; at most one packet may touch crates/legion-protocol/src/lib.rs; each packet owns at most six existing files.\n' +
  'Work that depends on an unavailable external prerequisite (macOS or Linux host, signing credentials, clean VMs, owner dogfood days, independent users, external security audit, a model download) goes in owner_blocked with the exact prerequisite string, never into packets.\n' +
  (nativeGuiResumed ? 'Native GUI work on this Windows host is permitted; set needs_native_gui true for it.\n' : 'Do not select work that requires launching a window.\n') +
  (focus ? 'Owner focus for this round: ' + focus + '\n' : '') +
  'Write each packet brief to ' + LEDGER + '/<packet_id>-brief.md in the style of the existing brief files: goal, owned files, files that must not be touched, exact test names, exact verification command, and the review focus.\n\n' +
  'Current state: ' + state.summary + '\nIn flight: ' + state.in_flight.join(', ') + '\nHandoff next work: ' + state.next_work.join(' | '),
  { label: 'planner', phase: 'Plan', schema: PLAN, effort: 'high' })

if (!plan || !plan.packets.length) return { round: round, halt: 'nothing selectable', plan: plan, owner_blocked: plan ? plan.owner_blocked : [] }

const seen = new Map()
const chokers = []
for (const p of plan.packets) {
  for (const f of own(p)) {
    if (seen.has(f)) return { round: round, halt: 'ownership overlap', file: f, packets: [seen.get(f), p.packet_id] }
    seen.set(f, p.packet_id)
    if (CHOKE.indexOf(f) >= 0 && chokers.indexOf(p.packet_id) < 0) chokers.push(p.packet_id)
  }
}
if (chokers.length > 1) return { round: round, halt: 'more than one chokepoint packet', chokers: chokers }
const packets = plan.packets.slice(0, maxPackets)
log('round ' + round + ': ' + packets.map(function (p) { return p.packet_id }).join(', '))

phase('Implement')
const impl = function (p, extra) {
  return agent(RULES + '\n\nRole: implementer (' + p.role + '). Packet ' + p.packet_id + ' - ' + p.title + '.\n' +
    'Read your brief at ' + p.brief + ' first and follow it exactly.\n' +
    'Goal: ' + p.goal + '\n' +
    'Edit ONLY these files: ' + own(p).join(', ') + '. Touching anything else fails the packet.\n' +
    'No cargo is available to you, so read the exact type and function signatures you depend on before using them rather than guessing.\n' +
    'Leave every file you touch rustfmt-clean. You MAY run rustfmt directly on your own files (for example rustfmt --edition 2024 <path>): rustfmt is not cargo, takes no build lock, and does not belong to the cargo lane. A round-level formatting gate failure caused by your files fails the whole round for everyone.\n' +
    'Add the named tests: ' + p.named_tests.join(', ') + '. Tests must assert real observable outcomes; never weaken an existing assertion or fixture to make something pass.\n' +
    'Write your report to ' + LEDGER + '/' + p.packet_id + '-report.md listing exactly what changed, what you tested, and every claim you could not verify (also put those in untested_claims).' +
    (extra || ''),
    { label: 'impl ' + p.packet_id, phase: 'Implement', schema: IMPL })
}
const impls = await parallel(packets.map(function (p) { return function () { return impl(p) } }))
let live = packets.filter(function (p, i) { return impls[i] && impls[i].status !== 'blocked' })
if (!live.length) return { round: round, halt: 'every implementer reported blocked', impls: impls }

phase('Check')
for (let attempt = 0; attempt < 3 && live.length; attempt++) {
  const crates = []
  for (const p of live) for (const c of p.crates) if (crates.indexOf(c) < 0) crates.push(c)
  const chk = await lane('check' + attempt,
    'Run one step per crate: cargo check -p <crate> --all-targets -j 1, for these crates: ' + crates.join(', ') + '.\n' +
    'Attribute every compiler error to the packet that owns the file it points at, using this ownership map: ' + JSON.stringify(live.map(function (p) { return [p.packet_id, own(p)] })) + '.\n' +
    'Put the verbatim diagnostics for each packet in that step diagnostics field. List packets with errors in failed_packets.',
    'Check')
  if (!chk) return { round: round, halt: 'cargo lane died', phase: 'Check' }
  if (!chk.failed_packets.length) break
  if (attempt === 2) {
    const doomed = live.filter(function (p) { return chk.failed_packets.indexOf(p.packet_id) >= 0 })
    log('reverting packets that did not compile after two fix attempts: ' + chk.failed_packets.join(', '))
    await lane('revert-check', 'Revert these packets and run no cargo: for each list of paths, git checkout -- the tracked files and delete the untracked new files: ' + JSON.stringify(doomed.map(own)) + '. Report the reverted paths in notes.', 'Check')
    live = live.filter(function (p) { return chk.failed_packets.indexOf(p.packet_id) < 0 })
    break
  }
  const failing = live.filter(function (p) { return chk.failed_packets.indexOf(p.packet_id) >= 0 })
  await parallel(failing.map(function (p) {
    return function () {
      const diags = chk.steps.filter(function (s) { return s.packet_id === p.packet_id }).map(function (s) { return s.diagnostics }).join('\n')
      return impl(p, '\n\nYour previous edit did not compile. Fix these exact compiler diagnostics (attempt ' + (attempt + 1) + ') without widening scope:\n' + diags)
    }
  }))
}
if (!live.length) return { round: round, halt: 'no packet compiled', packets: packets.map(function (p) { return p.packet_id }) }

phase('Test')
const packetTestBody = function (set, tag) {
  return [
    'For each packet run its named tests and report one step per packet with that packet_id: ' +
    JSON.stringify(set.map(function (p) { return { packet: p.packet_id, crates: p.crates, tests: p.named_tests, real_server: p.needs_real_server, brief: p.brief } })) + '.',
    'Use cargo test -p <crate> -j 1 --no-fail-fast with the test target or name filter given in that packet brief. Read the brief if the filter is unclear.',
    'Any packet marked real_server also runs: cargo test -p legion-app --test typescript_app_startup --test python_app_startup -j 1 --no-fail-fast -- --ignored --nocapture, with LEGION_TEST_NODE_RUNTIME exported.',
    'After a packet tests pass, run its lint step in the same lane: cargo clippy -p <crate> --all-targets -j 1 -- -D warnings, and cargo fmt --all --check once for the round. Report these as steps carrying that packet_id and add the packet to failed_packets when its lint fails. Catching a mechanical lint here costs one fix pass; catching it at the round gate fails the whole round for every packet.',
    'A packet whose named tests cannot even be compiled or filtered is a failure, not a skip. List every packet whose tests did not fully pass in failed_packets.',
    'Tag for log file names: ' + tag + '.',
  ].join('\n')
}
let test = await lane('test', packetTestBody(live, 'packet-tests'), 'Test')
if (!test) return { round: round, halt: 'cargo lane died', phase: 'Test' }

if (test.failed_packets.length) {
  const failing = live.filter(function (p) { return test.failed_packets.indexOf(p.packet_id) >= 0 })
  log('packets failing their own tests, one fix attempt: ' + test.failed_packets.join(', '))
  await parallel(failing.map(function (p) {
    return function () {
      const diags = test.steps.filter(function (s) { return s.packet_id === p.packet_id }).map(function (s) { return s.summary + '\n' + s.diagnostics }).join('\n')
      return impl(p, '\n\nYour named tests failed. Read the raw log named in the diagnostics below, then fix the cause. Do not weaken, delete, or narrow any assertion, fixture, or oracle to make a test pass; if the test is right and the implementation is wrong, fix the implementation.\n' + diags)
    }
  }))
  const retest = await lane('retest', packetTestBody(failing, 'packet-retests'), 'Test')
  if (!retest) return { round: round, halt: 'cargo lane died', phase: 'Retest' }
  const stillFailing = retest.failed_packets
  test = { steps: test.steps.filter(function (s) { return test.failed_packets.indexOf(s.packet_id) < 0 }).concat(retest.steps), failed_packets: stillFailing, notes: test.notes + ' | ' + retest.notes }
  live = live.filter(function (p) { return stillFailing.indexOf(p.packet_id) < 0 })
}
if (!live.length) return { round: round, halt: 'no packet passed its own tests', failed: test.failed_packets }

phase('Review')
const logFor = function (p) {
  const s = test.steps.filter(function (x) { return x.packet_id === p.packet_id })[0]
  return s ? s.log : '(no test log recorded)'
}
const reviewPrompt = function (p, pass) {
  return RULES + '\n\nRole: independent reviewer' + (pass > 1 ? ' (re-review pass ' + pass + '; you wrote neither the packet nor any earlier review)' : '') + '. You did NOT write packet ' + p.packet_id + ' and you must not fix it.\n' +
    'The cargo lane has ALREADY run this packet named tests. Read the raw test log at ' + logFor(p) + ' as part of your review; execution evidence exists and absence of it is not a finding.\n' +
    'Read the brief ' + p.brief + ', the report ' + LEDGER + '/' + p.packet_id + '-report.md (plus any fix reports), and the diff: git diff -- ' + own(p).join(' ') + ' and any new files.\n' +
    'Try to refute the report. Check specifically: the tests in the log actually exercise the claimed behaviour rather than restating the implementation; no fixture, assertion, or oracle was weakened; the change stays inside the brief scope; no chokepoint file grew; no PATH discovery or implicit executable grant; no fabricated proposal, approval, or evidence; no claim inflation in prose; anything unverifiable on this host is disclosed as blocked with an exact prerequisite rather than claimed.\n' +
    'Classify every finding. blocking_findings are defects that make the change wrong, unsafe, untruthful, or out of scope. nonblocking_findings are improvements, prose nits, and follow-ups that do not make the change wrong. Return verdict "changes" ONLY when blocking_findings is non-empty. If every finding is non-blocking, return "approve" and leave the follow-ups in nonblocking_findings; they will be recorded in the ledger, not lost. Return "reject" only when the packet should be abandoned rather than fixed.\n' +
    'Write your review to ' + LEDGER + '/' + p.packet_id + (pass > 1 ? '-fix' + (pass - 1) + '-review.md' : '-review.md') + ' and return its path.\n' +
    'proposed_implementation maps each of these requirement ids to one of absent, partial, implemented, repair-required: ' + p.requirement_ids.join(', ') + '. Propose a value only when the diff and its test log establish it; if the row already holds the right value, repeat it. You may not propose an acceptance value.'
}

let verdicts1 = await parallel(live.map(function (p) {
  return function () { return agent(reviewPrompt(p, 1), { label: 'review ' + p.packet_id, phase: 'Review', schema: REVIEW }) }
}))
let results = live.map(function (p, i) { return { p: p, r: verdicts1[i] } })

for (let pass = 2; pass <= 3; pass++) {
  const needFix = results.filter(function (x) { return x.r && x.r.verdict === 'changes' })
  if (!needFix.length) break
  log('review pass ' + (pass - 1) + ' requires blocking fixes: ' + needFix.map(function (x) { return x.p.packet_id }).join(', '))
  await parallel(needFix.map(function (x) {
    return function () {
      return impl(x.p, '\n\nAn independent review found BLOCKING defects. Apply these findings exactly, without arguing and without widening scope: ' + JSON.stringify(x.r.blocking_findings) +
        '\nThese non-blocking follow-ups are recorded but you should NOT act on them unless one is trivially adjacent to a blocking fix: ' + JSON.stringify(x.r.nonblocking_findings) +
        '\nWrite the follow-up report to ' + LEDGER + '/' + x.p.packet_id + '-fix' + (pass - 1) + '-report.md.')
    }
  }))
  const fixTest = await lane('fixtest' + (pass - 1), packetTestBody(needFix.map(function (x) { return x.p }), 'fix' + (pass - 1) + '-tests'), 'Test')
  if (!fixTest) return { round: round, halt: 'cargo lane died', phase: 'Review fix test' }
  test = { steps: test.steps.concat(fixTest.steps), failed_packets: fixTest.failed_packets, notes: test.notes }
  const brokeIt = fixTest.failed_packets
  const reviewable = needFix.filter(function (x) { return brokeIt.indexOf(x.p.packet_id) < 0 })
  const newVerdicts = await parallel(reviewable.map(function (x) {
    return function () { return agent(reviewPrompt(x.p, pass), { label: 're-review ' + x.p.packet_id, phase: 'Review', schema: REVIEW }) }
  }))
  results = results.map(function (x) {
    if (brokeIt.indexOf(x.p.packet_id) >= 0) return { p: x.p, r: { verdict: 'reject', blocking_findings: ['fix pass ' + (pass - 1) + ' broke the packet own tests'], nonblocking_findings: [], proposed_implementation: {}, review_path: '', rationale: 'tests failed after fix' } }
    const idx = reviewable.map(function (y) { return y.p.packet_id }).indexOf(x.p.packet_id)
    return idx >= 0 ? { p: x.p, r: newVerdicts[idx] } : x
  })
}

const approved = results.filter(function (x) { return x.r && x.r.verdict === 'approve' })
const rejected = live.filter(function (p) { return !approved.some(function (a) { return a.p.packet_id === p.packet_id }) })
if (!approved.length) return { round: round, halt: 'no packet passed independent review', rejected: rejected.map(function (p) { return p.packet_id }), reviews: results }

phase('Gates')
const gateBody = [
  'Step 1 (no cargo): preserve then revert every rejected packet so its work is not lost. For each packet id and its paths ' + JSON.stringify(rejected.map(function (p) { return [p.packet_id, own(p)] })) + ': first write git diff -- <its tracked paths> to ' + LEDGER + '/<packet_id>-rejected.patch, and copy any untracked new files it created into ' + LEDGER + '/<packet_id>-rejected-newfiles/. Only then git checkout -- the tracked paths and delete the untracked new files. Report the patch paths in notes.',
  'Step 2 regression guard: cargo test -p legion-app --lib -j 1 (expect 439 or more, none failing) and cargo test -p legion-desktop --lib -j 1 (expect 243 or more, none failing). Report the exact counts. A drop is a regression: report it, do not hide or revert it silently.',
  'Step 3: cargo clippy --workspace --all-targets -j 1 -- -D warnings.',
  'Step 4: fast gates in order, each its own step: cargo fmt --all --check; cargo run -p xtask -- check-deps; cargo run -p xtask -- docs-hygiene; cargo run -p xtask -- claim-audit; cargo run -p xtask -- extract-before-modify.',
  'Every step here is round-level: set packet_id to the empty string.',
].join('\n')
const gates = await lane('gates', gateBody, 'Gates')
if (!gates) return { round: round, halt: 'cargo lane died', phase: 'Gates' }
const roundGateFailures = gates.steps.filter(function (s) { return !s.claimed_pass })
if (roundGateFailures.length) return { round: round, halt: 'round gate failed', failures: roundGateFailures, approved: approved.map(function (a) { return a.p.packet_id }) }

phase('Refute')
const claims = test.steps.concat(gates.steps).filter(function (s) { return s.claimed_pass })
const verdicts = await parallel(claims.map(function (s) {
  return function () {
    return agent('Try to refute this claimed pass. Start from the raw log at ' + s.log + ', which should carry a CMD/CWD/BEGIN/output/END/EXIT provenance shape.\n' +
      'Apply the criteria that fit the kind of step this is, not test criteria to a non-test step.\n' +
      'For a step that ran tests: the named tests appear with ok, the "test result:" line shows zero failed, and the counts match the claim.\n' +
      'For a lint, format, or gate step: the CMD line names the command the claim says was run, and the body plus EXIT are consistent with the claimed outcome. A tool that passes silently legitimately has an empty body; that is only acceptable when CMD, CWD, BEGIN, END and EXIT frame it, because those prove invocation. A bare exit marker with no command echo is not evidence.\n' +
      'In every case: nothing says the run was interrupted or could not compile, and the EXIT marker belongs to the claimed command.\n' +
      'You may run your own read-only verification commands to check a claim about repository state (git status, git diff --stat, ls, cat of a review or ledger file). You may NOT run cargo, rustc, or xtask. Prefer checking the world over re-reading the claim.\n' +
      'Default refuted=true when uncertain, when the log does not contain what the claim says, or when the only support for a claim is the claimant restating it. Claim: ' + JSON.stringify(s),
      { label: 'refute ' + s.kind + ' ' + (s.packet_id || 'round'), phase: 'Refute', schema: VERDICT, effort: 'high' })
  }
}))
const refuted = claims.filter(function (s, i) { return !verdicts[i] || verdicts[i].refuted })
const refutedReasons = verdicts.filter(function (v) { return v && v.refuted }).map(function (v) { return v.reason })
const refutedRoundLevel = refuted.filter(function (s) { return !s.packet_id })
if (refutedRoundLevel.length) {
  return { round: round, halt: 'round-level claimed pass refuted', refuted: refutedRoundLevel, reasons: refutedReasons }
}
const refutedPackets = refuted.map(function (s) { return s.packet_id })
if (refutedPackets.length) log('packet claims refuted, those packets do not commit this round: ' + refutedPackets.join(', '))
const passed = approved.filter(function (a) {
  return test.failed_packets.indexOf(a.p.packet_id) < 0 && refutedPackets.indexOf(a.p.packet_id) < 0
})
if (!passed.length) return { round: round, halt: 'no packet passed its tests', failed: test.failed_packets }

phase('Record')
const rec = await agent(RULES + '\n\nRole: record. You are the only committer and the only editor of plans/completion/requirements.json this round. Run no cargo.\n' +
  'Passed packets with their reviewer-proposed implementation transitions and owned paths: ' + JSON.stringify(passed.map(function (a) { return { packet: a.p.packet_id, title: a.p.title, requirements: a.p.requirement_ids, transitions: a.r.proposed_implementation, paths: own(a.p) } })) + '\n' +
  'Packets whose tests failed (already reverted or left uncommitted; their register rows do not change): ' + JSON.stringify(test.failed_packets) + '\n' +
  'Packets rejected at review: ' + JSON.stringify(rejected.map(function (p) { return p.packet_id })) + '. Their work was saved as ' + LEDGER + '/<packet_id>-rejected.patch before the tree was reverted; record that path in the ledger entry so a later round can resume from it instead of starting over.\n' +
  'Non-blocking review follow-ups to record in the ledger (do not act on them, just carry them forward): ' + JSON.stringify(approved.map(function (a) { return { packet: a.p.packet_id, follow_ups: a.r.nonblocking_findings } })) + '\n' +
  'Owner-blocked prerequisites found this round: ' + JSON.stringify(plan.owner_blocked) + '\n\n' +
  'Do these in order:\n' +
  '1. Apply the implementation transitions to ' + WT + '/plans/completion/requirements.json, preserving every other field on every row. Verify before and after that the row count is 419 and that only the intended implementation values changed. Never touch acceptance.\n' +
  '2. Copy the round raw logs ' + LEDGER + '/round-' + round + '-*.log into ' + EVID + '/ and append that directory README.md with one line per log giving the command, the exit code, and the classification (component or integrated evidence; not packaged, native GUI, or product acceptance).\n' +
  '3. Append ' + LEDGER + '/progress.md with one entry per packet and one round entry, each ending in a "Ruling: ... Cost if wrong: ..." line, and the standing line that the goal remains unfinished, not blocked or complete. Update ' + LEDGER + '/packets.json with this round and each packet final state. Add any new owner-blocked prerequisites to ' + LEDGER + '/blockers.json and to the owner-blocked section of ' + WT + '/plans/completion/decisions.md.\n' +
  '4. Confirm from the test log that every fast gate step exited 0. If any did not, commit nothing and report it as an escalation.\n' +
  '5. Make one code commit per passed packet, staging only that packet owned and new files, with subject "[<packet_id>] <imperative summary>". Then make one separate record commit staging plans/completion/requirements.json, plans/completion/decisions.md, and the evidence directory, with subject "[record ' + round + '] Record round ' + round + ' status and evidence". Never mix code and record files in one commit.\n' +
  (pushAuthorized
    ? '6. Push to origin/codex/full-product-resume. If no pull request is open for this branch, open one against main with gh describing the packets, the evidence classification, and the standing owner-blocked list; return its URL. Never merge.\n' +
      '7. MANDATORY, do not skip: after pushing, read the hosted three-OS gate outcome for the head you just pushed. Poll with gh run list --workflow "Legion Gates" --branch codex/full-product-resume and gh pr checks 218 until the run for that head completes, then record its per-job conclusion in the ledger entry and in notes. A local Windows pass is not evidence the branch is green; only the hosted run is. If any job failed, name the failing test and put it in escalations as the next round highest-priority work. Nine consecutive hosted failures once went unnoticed because this step did not exist.\n'
    : '6. Do not push and do not open a pull request.\n') +
  'Stamp ' + stamp + '.',
  { label: 'record', phase: 'Record', schema: RECORD })

return {
  round: round, stamp: stamp, mode: mode,
  passed: passed.map(function (a) { return a.p.packet_id }),
  failed: test.failed_packets,
  rejected: rejected.map(function (p) { return p.packet_id }),
  owner_blocked: plan.owner_blocked,
  escalations: plan.escalations.concat(rec ? rec.escalations : ['record agent died: verified work is on disk but uncommitted']),
  commits: rec ? rec.commits : [],
  pr_url: rec ? rec.pr_url : '',
}
