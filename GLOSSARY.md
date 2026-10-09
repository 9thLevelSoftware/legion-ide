# Legion IDE

Legion combines manual editing with optional language assistance and delegated
work. Workspace changes remain subject to the applicable review and policy.

## Language

**Worktree search**:
A delegated task's search for file paths or matching text within its permitted
worktree, excluding forbidden paths.

**LSP interaction scheduling**:
The timing of completion and hover requests following accepted editing and
cursor actions, using the resulting editor position.

**Explorer activation**:
Opening a file selected in the explorer and selecting its row after opening
succeeds. Activating a directory instead toggles its expanded state.

### Change review and evidence

**Workspace proposal**:
A reviewable request to change a workspace, identifying the intended changes
and the conditions under which they may be applied.
_Avoid_: Authorized write, verified change

**Proposal approval**:
A review decision accepting an exact workspace proposal under its required
checks and relevant editor and dependency state, subject to workspace policy.
_Avoid_: Verification success, unconditional permission

**Required check**:
A check explicitly mandated by authorized workspace policy or attached by the
developer to a proposal, whose requirement must be satisfied before application.
_Avoid_: Optional check, suggested check

**Optional check**:
A check whose evidence informs proposal review without making its success a
condition of application.
_Avoid_: Required check, implicit gate

**Proposed editor state**:
The workspace state formed by the proposed changes together with relevant
unsaved edits, which may differ from the state on disk after application.
_Avoid_: Saved workspace, disk state

**Verification subject**:
The exact proposed workspace state, named properties, dependencies, assumptions,
and checking conditions to which a verification result applies.
_Avoid_: Current code, candidate without qualification

**Verification run**:
One attempt to check a verification subject; a recheck is a new attempt even
when the subject is unchanged.
_Avoid_: Qualification run, proof

**Verification activation**:
Explicit authorization for a verification or repair operation within its stated
scope, execution permissions, and aggregate budget, including internal attempts.
_Avoid_: Unlimited retry consent, proposal approval

**Verification evidence**:
The recorded outcome and supporting observations of a verification run,
limited to its subject and stated coverage.
_Avoid_: Approval, safety certificate

**Evidence freshness**:
Whether verification evidence still applies to the subject currently under
review, independently of whether the check succeeded or covered all of it.
_Avoid_: Success, coverage, availability

**Verification coverage**:
The portion of the declared verification scope actually checked, with known
omissions and unknown coverage distinguished from completeness.
_Avoid_: Freshness, test coverage without qualification

**Validated counterexample**:
A witness established to violate a named property of the verification subject
under the stated assumptions and checking conditions.
_Avoid_: Tool failure, unproved obligation, arbitrary failing output

**Verified refactoring**:
A proposed transformation independently shown to preserve the stated semantics
within its supported domain and checking conditions.
_Avoid_: Universally safe edit, approved change

**Qualification run**:
An evaluation of a particular Legion build against a named acceptance scenario
and configuration, supporting a product-readiness decision.
_Avoid_: Verification run, product certification

**Internal daily-driver pilot**:
A bounded internal evaluation of Legion on real edit, test, debug, Git, and
change-review work; its acceptance applies to the declared pilot scope.
_Avoid_: Production release, enterprise certification

**Manual daily-driver acceptance**:
Demonstrated completion of the declared ordinary editing and development
workflows without requiring AI-provider availability.
_Avoid_: Assisted-workflow acceptance, whole-program acceptance

**Assisted-workflow acceptance**:
Demonstrated completion of the declared AI-assisted workflows, including real
provider behavior, review, cancellation and recovery within their authority limits.
_Avoid_: Manual daily-driver acceptance, deterministic-provider wiring

### Agent integration and migration

**Agent-session recovery**:
Restoration of retained agent-work context and progress after interruption,
with interrupted execution stopped until explicitly resumed.
_Avoid_: Background execution, desktop layout restoration

**Qualified agent integration**:
A named external agent and capability set demonstrated to work within Legion's
review, workspace-authority, privacy and execution boundaries.
_Avoid_: Protocol compatibility alone, unrestricted agent support

**Migration coverage**:
The declared settings, keybindings and extensions whose transfer or equivalent
behavior has been demonstrated in Legion, with unsupported items identified.
_Avoid_: Universal VS Code parity, manifest parsing alone

### Change intelligence

**Resolved code relationship**:
A relationship whose endpoints are identified declarations within a stated
language and dependency context, rather than inferred from matching names.
_Avoid_: Name match, heuristic relationship

**Impact coverage**:
The completeness of known change-impact relationships within a declared scope;
an absent relationship outside complete coverage does not establish no impact.
_Avoid_: Verification coverage, global completeness

**Derived canvas relationship**:
A displayed relationship supported by code intelligence and its provenance,
distinct from a connection drawn by a person.
_Avoid_: User annotation, proof of dependency
