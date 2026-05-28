# Legion IDE Design System

_A starting style guide for a fast, native-feeling, AI-native development environment._

---

## 1. Product Identity

**Product name:** Legion IDE  
**Category:** AI-native software development environment  
**Core concept:** A desktop IDE that lets developers fluidly move from manual coding to autonomous multi-agent software execution.

Legion IDE should not look or feel like a VS Code fork, a generic Electron app, or a chat sidebar bolted onto a code editor. It should feel native, fast, elegant, operational, and deeply purpose-built for commanding AI software teams.

### Core Promise

> From code editor to autonomous engineering fleet.

### Product Personality

Legion IDE should feel:

- Fast
- Native
- Technical
- Calm
- Precise
- Command-driven
- Auditable
- Trustworthy
- Premium
- Developer-first

Legion IDE should not feel:

- Cluttered
- Toy-like
- Chatbot-first
- Sci-fi gimmicky
- Overly colorful
- Bloated
- Enterprise-dull
- Like a VS Code clone

---

## 2. Design Principles

### 2.1 Code First, AI Native

Even in autonomous modes, the product should respect the developer’s mental model. Code, diffs, tests, approvals, and project context remain visible and inspectable.

### 2.2 Autonomy Is a Spectrum

The UI is organized around a five-level autonomy model. Each level changes the interface, the AI affordances, and the developer’s role.

| Level | Name | Developer Role | AI Role |
|---:|---|---|---|
| 1 | Manual | Writes code directly | Quiet, optional |
| 2 | Assisted | Writes code with inline help | Suggests and explains |
| 3 | Co-Pilot | Collaborates on edits | Proposes multi-file changes |
| 4 | Delegated | Reviews scoped tasks | Executes assigned tasks |
| 5 | Autonomous Fleet | Directs and supervises | Plans, executes, tests, reviews |

### 2.3 Human Control Is Always Visible

Higher autonomy should feel powerful, not reckless. The UI must clearly show:

- Current autonomy level
- Active agents
- Current directive
- Files being changed
- Tests being run
- Decisions made
- Human approvals required
- Risk level
- Recovery path when something fails

### 2.4 Transparency Without Chain-of-Thought

Show concise summaries of agent decisions, actions, and outcomes. Do not expose raw hidden reasoning. The user should understand what happened and why without being overwhelmed.

### 2.5 Calm Operational Density

The interface can be dense, but it should not be noisy. Use compact rows, subtle dividers, restrained color, and clear hierarchy.

---

## 3. Visual Direction

### Inspiration

Use these references as directional inspiration, not as templates:

- **Zed:** Native speed, minimal code-first interface
- **Linear:** Elegant workflow boards, dark mode, density, precision
- **Raycast:** Command-driven interactions, fast keyboard UX
- **Arc:** Premium app polish and restrained personality

### Visual Keywords

- Dark
- Minimal
- Precise
- Monochromatic
- Sharp
- Layered
- Technical
- Quietly futuristic
- High contrast where needed
- Subtle glow only for AI/autonomy states

---

## 4. Color System

### 4.1 Base Palette

Use a deep dark palette with very small shifts between surfaces.

| Token | Hex | Usage |
|---|---:|---|
| `--bg-root` | `#0D0D12` | App background |
| `--bg-base` | `#111118` | Primary panels |
| `--bg-raised` | `#15151F` | Raised panels, cards |
| `--bg-elevated` | `#1A1A24` | Modals, popovers, active panels |
| `--bg-hover` | `#20202B` | Hover state |
| `--bg-selected` | `#252535` | Selected row/card |
| `--bg-input` | `#101018` | Inputs, command fields |
| `--bg-code` | `#0B0B10` | Code editor |

### 4.2 Border Palette

Borders should be subtle and low-opacity.

| Token | Value | Usage |
|---|---:|---|
| `--border-subtle` | `rgba(255,255,255,0.05)` | Pane separators |
| `--border-default` | `rgba(255,255,255,0.08)` | Cards, inputs |
| `--border-strong` | `rgba(255,255,255,0.14)` | Active elements |
| `--border-focus` | `rgba(107, 92, 255, 0.65)` | Focus ring |

### 4.3 Text Palette

| Token | Hex / Value | Usage |
|---|---:|---|
| `--text-primary` | `#F4F4F6` | Main text |
| `--text-secondary` | `#B6B7C3` | Labels, secondary text |
| `--text-muted` | `#7E8190` | Hints, metadata |
| `--text-disabled` | `#555868` | Disabled controls |
| `--text-inverse` | `#09090D` | Text on bright accents |

### 4.4 Accent Palette

Use accents sparingly. They should communicate system state, not decorate the UI.

| Token | Hex | Usage |
|---|---:|---|
| `--accent-cyan` | `#39D7FF` | Assisted AI, execution, active links |
| `--accent-blue` | `#4B8CFF` | Writing, code actions |
| `--accent-violet` | `#8B5CFF` | Autonomy, review, fleet activity |
| `--accent-purple` | `#B16CFF` | Level 5 emphasis, premium glow |
| `--accent-amber` | `#FFCC66` | Planning, waiting, attention |
| `--accent-green` | `#4ADE80` | Passed, complete, healthy |
| `--accent-red` | `#FF5C7A` | Failed, error, destructive |

### 4.5 Status Colors

| Status | Color | Usage |
|---|---:|---|
| Idle | `#6B6E7D` | Dormant agent or inactive state |
| Thinking | `#F5B85B` | Model processing |
| Planning | `#FFCC66` | Task planning |
| Writing | `#4B8CFF` | Code generation |
| Reviewing | `#8B5CFF` | Review or audit |
| Testing | `#39D7FF` | Test execution |
| Passed | `#4ADE80` | Success |
| Failed | `#FF5C7A` | Failure |
| Waiting on Human | `#FFB86B` | Approval needed |

### 4.6 Gradients and Glow

Use glow only around autonomy controls, active fleet states, and important AI indicators.

```css
--glow-cyan: 0 0 24px rgba(57, 215, 255, 0.22);
--glow-violet: 0 0 28px rgba(139, 92, 255, 0.26);
--glow-fleet: 0 0 32px rgba(177, 108, 255, 0.30), 0 0 18px rgba(57, 215, 255, 0.16);
```

Avoid heavy shadows. Prefer subtle background elevation and low-opacity borders.

---

## 5. Typography

### 5.1 Font Stack

```css
--font-ui: Inter, SF Pro Display, SF Pro Text, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
--font-code: "JetBrains Mono", "Berkeley Mono", "Geist Mono", "SF Mono", Consolas, monospace;
```

### 5.2 Type Scale

| Token | Size | Line Height | Weight | Usage |
|---|---:|---:|---:|---|
| `--text-xs` | 11px | 16px | 400–500 | Metadata, badges |
| `--text-sm` | 12px | 18px | 400–500 | Sidebar labels, cards |
| `--text-md` | 13px | 20px | 400–500 | Default UI text |
| `--text-lg` | 15px | 22px | 500–600 | Panel headers |
| `--text-xl` | 18px | 26px | 600 | Section titles |
| `--text-2xl` | 24px | 32px | 600–700 | Empty states, onboarding |

### 5.3 Code Typography

| Token | Value |
|---|---:|
| Code size | 13px |
| Code line height | 20px |
| Terminal size | 12px |
| Terminal line height | 18px |
| Letter spacing | Normal or slightly tight |

Code should be crisp and readable. Avoid overly decorative syntax themes.

---

## 6. Spacing and Layout Tokens

### 6.1 Spacing Scale

```css
--space-1: 4px;
--space-2: 8px;
--space-3: 12px;
--space-4: 16px;
--space-5: 20px;
--space-6: 24px;
--space-8: 32px;
--space-10: 40px;
--space-12: 48px;
```

### 6.2 Radius Scale

```css
--radius-sm: 6px;
--radius-md: 8px;
--radius-lg: 12px;
--radius-xl: 16px;
--radius-pill: 999px;
```

### 6.3 Layout Dimensions

| Region | Default Size |
|---|---:|
| Top app bar | 48–56px |
| Left sidebar | 260–300px |
| Right directive console | 340–420px |
| Bottom panel | 220–320px expanded |
| Icon rail compact sidebar | 48–56px |
| Kanban card width | 260–320px |
| Kanban column width | 300–360px |

---

## 7. Global App Shell

### 7.1 Top App Bar / Command Center

The top app bar is the control spine of the product.

Required elements:

- Legion logo mark
- Workspace name
- Git branch
- Legion Engine status
- Centered Autonomy Scale
- Command palette shortcut
- Build/test status
- Resource indicators
- Run Directive / Pause Fleet button
- User or workspace avatar

Style:

- Height: 48–56px
- Background: `--bg-base`
- Bottom border: `--border-subtle`
- Compact controls
- No heavy toolbar styling

### 7.2 Left Sidebar

The left sidebar balances project navigation and AI fleet management.

At low autonomy levels:

- File explorer dominates
- Active Fleet is collapsed or minimized

At high autonomy levels:

- Active Fleet dominates
- File explorer becomes secondary or collapsible

### 7.3 Main Canvas

The main canvas changes by autonomy level.

| Level | Main Canvas Focus |
|---:|---|
| 1 | Full code editor |
| 2 | Code editor with inline AI assistance |
| 3 | Editor + multi-file AI diff |
| 4 | Diff review + delegated task board |
| 5 | Autonomous Kanban board |

### 7.4 Right Sidebar / Directive Console

The right sidebar is the human command and supervision interface.

It should contain:

- Directive input
- Current objective
- Context scope
- Approval queue
- Agent decision feed
- Risk monitor

At Levels 1–2, it may be collapsed or slim. At Levels 3–5, it becomes central to the workflow.

### 7.5 Bottom Panel

The bottom panel contains operational output:

- Terminal
- Tests
- Agent Comm Stream
- Workflow logs
- Build output

At Level 5, the Agent Comm Stream becomes especially important.

---

## 8. Autonomy Scale

The Autonomy Scale is the defining UI component of Legion IDE.

### 8.1 Component Requirements

- Always visible in the top-center command bar
- Segmented control combined with slider behavior
- Five labeled levels
- Active state must be instantly legible
- Higher autonomy states should feel more energetic
- Levels 4 and 5 should communicate increased capability and increased responsibility

### 8.2 Labels

```text
1 Manual
2 Assisted
3 Co-Pilot
4 Delegated
5 Fleet
```

### 8.3 Microcopy

| Level | Label | Microcopy |
|---:|---|---|
| 1 | Manual | You write. AI stays quiet. |
| 2 | Assisted | AI helps inline. |
| 3 | Co-Pilot | AI pairs with you. |
| 4 | Delegated | Agents handle scoped tasks. |
| 5 | Fleet | The fleet executes directives. |

### 8.4 Visual States

| Level | Color Treatment | UI Energy |
|---:|---|---|
| 1 | Muted gray/blue | Quiet |
| 2 | Soft cyan | Light assistance |
| 3 | Blue-violet | Collaborative |
| 4 | Violet/cyan | Operational |
| 5 | Electric violet + cyan glow | Autonomous, active |

### 8.5 Safety Interaction

Switching into Level 4 or Level 5 should show a confirmation flow unless the user has disabled confirmations.

Level 4 confirmation:

```text
Enter Delegated Mode?
Agents can work on scoped tasks and prepare diffs for your review.
```

Level 5 confirmation:

```text
Activate Autonomous Fleet?
The fleet will break down directives, modify files, run tests, and prepare changes for review.
```

Include clear permission toggles:

- Require approval before applying edits
- Auto-run tests
- Allow terminal commands
- Allow dependency installation
- Scope: selected files / current module / entire repo
- Risk tolerance: Conservative / Balanced / Aggressive

---

## 9. Autonomy-Level UI Behavior

### 9.1 Level 1 — Manual

Human writes code directly. AI is quiet.

UI emphasis:

- Full editor
- File tree
- Terminal
- Optional tiny AI affordance

Do:

- Keep AI subtle
- Use minimal accent color
- Prioritize editor performance and readability

Do not:

- Show a large chat panel
- Show agent boards
- Over-emphasize AI

### 9.2 Level 2 — Assisted

AI provides inline completions, fixes, and explanations.

UI emphasis:

- Editor remains primary
- Ghost text completions
- Contextual action popovers
- Slim assistant panel

Example actions:

- Explain selection
- Generate docstring
- Suggest fix
- Generate test
- Refactor selection

### 9.3 Level 3 — Co-Pilot

AI actively pairs with the developer.

UI emphasis:

- Multi-file diff
- Co-pilot plan
- Accept/reject controls
- Pair session panel

Example UI elements:

- “AI Plan” strip
- Proposed changes grouped by file
- Apply all
- Request revision
- Human feedback input

### 9.4 Level 4 — Delegated

Developer delegates scoped tasks to agents.

UI emphasis:

- Active Fleet sidebar
- Diff review area
- Delegated task board
- Human approval queue

Key states:

- Assigned
- In Progress
- Waiting on Human
- Testing
- Done

### 9.5 Level 5 — Autonomous Fleet

Developer gives a master directive and supervises a multi-agent fleet.

UI emphasis:

- Autonomous Kanban board
- Master Directive bar
- Active Fleet
- Directive Console
- Agent Comm Stream
- Risk Monitor

The developer is not writing every line. The developer is directing, reviewing, constraining, and approving.

---

## 10. Component Guidelines

## 10.1 Buttons

### Primary Button

Used for high-value actions.

Examples:

- Run Directive
- Activate Fleet
- Review & Merge
- Approve Changes

Style:

```css
background: linear-gradient(135deg, #39D7FF 0%, #8B5CFF 100%);
color: #09090D;
border-radius: 8px;
font-weight: 600;
```

Use sparingly. There should usually be only one primary action per region.

### Secondary Button

Used for normal actions.

Examples:

- Run Tests
- Open Diff
- Add Constraint
- Request Revision

Style:

- Dark raised background
- Low-opacity border
- Text primary or secondary

### Destructive Button

Used for destructive or stop actions.

Examples:

- Reject Changes
- Stop Fleet
- Remove Agent

Use red text or red border. Avoid large filled red buttons unless urgent.

---

## 10.2 Cards

Cards are used for tasks, approvals, agents, and summaries.

Style:

- Background: `--bg-raised`
- Border: `--border-default`
- Radius: `--radius-lg`
- Padding: 12–16px
- Hover: slightly brighter background and stronger border

Cards should be compact and information-rich.

---

## 10.3 Status Chips

Status chips communicate state quickly.

Examples:

```text
Manual Mode
Assisted Coding Active
Pair Programming Active
Delegated Tasks Active
Autonomous Fleet Active
Fleet Paused
Intervention Needed
Awaiting Human Review
Directive Completed
```

Style:

- Pill shape
- Small text
- Colored dot or subtle tinted background
- Avoid large badges

---

## 10.4 Agent Rows

Agent rows appear in the Active Fleet sidebar and task details.

Each row should include:

- Agent/model icon
- Role
- Current status
- Current task
- Small progress indicator
- Optional activity pulse

Example:

```text
GPT-5.5   Backend Agent   Writing   checkout.ts   68%
```

---

## 10.5 Kanban Cards

Kanban cards are central to Level 4 and Level 5.

Each task card should include:

- Task title
- Assigned agent/team
- Model badge
- Status chip
- Progress bar
- Files touched
- Risk level
- Test status
- Mini code/diff preview
- Last activity timestamp

Example:

```text
Implement checkout session endpoint
Backend Team · GPT-5.5 · Executing
Progress: 68%
Files: api/billing/checkout.ts, services/stripe.ts
Risk: Medium
Tests: 4 generated
+ const session = await stripe.checkout.sessions.create(...)
```

---

## 10.6 Directive Input

The Directive Input should feel like a command palette and mission briefing field.

Placeholder:

```text
Describe what you want built, changed, tested, or reviewed…
```

Example:

```text
Implement Stripe subscriptions and wire them into the existing user model.
```

Actions:

- Run
- Delegate
- Add Constraint
- Attach Context
- Dry Run

---

## 10.7 Approval Queue Items

Approval queue items must be compact but clear.

Each item should include:

- Requested action
- Agent owner
- Risk badge
- Files touched
- Approve / Review / Reject actions

Example:

```text
Apply migration to user_subscriptions table
Backend Agent · Medium Risk · 3 files touched
[Approve] [Review] [Reject]
```

---

## 10.8 Agent Decision Feed

The decision feed shows concise summaries of what agents did or decided.

Do not show raw hidden reasoning.

Good examples:

```text
Backend Team selected Stripe checkout sessions over payment links.
QA Agent added canceled-subscription regression tests.
Review Agent flagged missing idempotency guard.
```

Bad examples:

```text
Here is my private chain-of-thought...
```

---

## 10.9 Agent Comm Stream

The Agent Comm Stream is a dense, technical event feed.

Example format:

```text
[12:04:11] [PLAN] Planner → Backend Team: Assigned checkout session endpoint
[12:04:18] [WRITE] Backend Agent: Added Stripe SDK integration
[12:04:31] [TEST] QA Agent: Generated 6 subscription lifecycle tests
[12:04:44] [REVIEW] Review Agent: Flagged missing webhook idempotency guard
[12:05:19] [COMPLETE] Test Runner: billing.test.ts passed 14/14
```

Tags:

- PLAN
- WRITE
- TEST
- REVIEW
- ERROR
- APPROVAL
- COMPLETE

---

## 11. Code and Diff Views

### 11.1 Code Editor

Style:

- Background: `--bg-code`
- Font: `--font-code`
- Font size: 13px
- Line height: 20px
- Subtle line numbers
- Minimal editor chrome
- No excessive gutter clutter

### 11.2 Diff View

Required elements:

- Original code
- Proposed code
- Inline change highlights
- File tabs
- Change summary bar
- Agent annotation bubbles
- Approval controls

Change colors:

```css
--diff-add-bg: rgba(74, 222, 128, 0.10);
--diff-add-border: rgba(74, 222, 128, 0.35);
--diff-remove-bg: rgba(255, 92, 122, 0.10);
--diff-remove-border: rgba(255, 92, 122, 0.35);
```

Annotation examples:

```text
Added Stripe signature verification
Introduced idempotency guard
Potential issue: missing retry handling
QA Agent generated regression test
```

---

## 12. Motion and Interaction

Motion should be subtle, fast, and native-feeling.

### 12.1 Timing

| Motion | Duration |
|---|---:|
| Hover transition | 100–140ms |
| Panel open/close | 160–220ms |
| Mode transition | 240–360ms |
| Toast entrance | 160ms |
| Glow pulse | 1800–2400ms loop |

### 12.2 Easing

Use ease-out or spring-like motion. Avoid bouncy consumer-app animation.

```css
--ease-standard: cubic-bezier(0.2, 0.8, 0.2, 1);
--ease-emphasized: cubic-bezier(0.16, 1, 0.3, 1);
```

### 12.3 Mode Transitions

When switching autonomy levels:

- Panels should resize smoothly
- Sidebars should expand/collapse gracefully
- New areas should fade and slide in subtly
- The Autonomy Scale active segment should animate first
- No jarring full-screen reflows

### 12.4 Live Agent Activity

Agent activity indicators should be subtle:

- Small pulsing dot
- Thin progress bar
- Slight row shimmer only if active
- No loud spinners everywhere

---

## 13. Accessibility

### 13.1 Contrast

- Primary text must meet WCAG AA contrast against dark backgrounds.
- Muted metadata can be lower contrast but must remain legible.
- Status cannot rely on color alone; include labels/icons.

### 13.2 Keyboard Navigation

Legion IDE should be keyboard-first.

Required shortcuts:

```text
⌘K / Ctrl+K       Open command palette
⌘Enter            Run directive / submit action
Esc               Dismiss popover/modal
Tab               Accept inline suggestion
⌘⇧A               Open Autonomy Scale
⌘⇧F               Focus fleet / agent panel
⌘⇧R               Review pending changes
```

### 13.3 Focus States

Focus rings should be visible but elegant.

```css
outline: 1px solid rgba(107, 92, 255, 0.75);
box-shadow: 0 0 0 3px rgba(107, 92, 255, 0.18);
```

---

## 14. Content and Microcopy

### 14.1 Voice

The voice should be:

- Precise
- Calm
- Operational
- Developer-native
- Direct
- Safety-aware

Avoid exaggerated AI hype.

### 14.2 Preferred Terms

Use:

- Directive
- Fleet
- Agent
- Task
- Diff
- Review
- Approval
- Context
- Scope
- Risk
- Tests
- Workflow

Avoid overusing:

- Magic
- Robot
- Brain
- Superhuman
- Revolutionary inside the UI

### 14.3 Common Actions

```text
Run Directive
Pause Fleet
Resume Fleet
Review Decisions
Approve Changes
Request Revision
Reject Changes
Run Tests
Open Diff
Apply All
Commit Changes
Create Pull Request
Add Constraint
Change Scope
Force Review
```

### 14.4 Safety Copy

```text
Require approval before applying edits
Allow agents to run tests
Allow agents to install dependencies
Allow terminal commands
Require approval for destructive commands
Limit scope to selected files
Protect environment files
Conservative risk mode
Human approval required
```

### 14.5 Example Master Directives

```text
Implement Stripe subscriptions and wire them into the existing user model.
Generate unit tests for the authentication flow and patch uncovered edge cases.
Refactor the billing API into service modules without changing external behavior.
Audit the repo for security issues and prepare a prioritized remediation plan.
Add role-based access control for admin, editor, and viewer users.
Prepare this app for production deployment with environment validation and health checks.
```

---

## 15. Empty, Error, and Success States

### 15.1 Empty Level 5 State

Title:

```text
Command the fleet
```

Subtitle:

```text
Give Legion a directive. The agent fleet will break it down, plan the work, execute changes, test, and prepare everything for review.
```

Idle stream message:

```text
Fleet standing by.
```

### 15.2 Error State

Tone: serious but controlled.

Status chip:

```text
Fleet Active — Intervention Needed
```

Actions:

- Review Failures
- Approve Patch
- Review Diff
- Stop Fleet

Show:

- Failed command
- Failed files
- Suspected cause
- Agent recovery plan
- Human approval request

### 15.3 Success State

Tone: calm and professional.

Status chip:

```text
Directive Completed
```

Actions:

- Open Full Diff
- Run Tests Again
- Commit Changes
- Create Pull Request
- Request Revision

Show:

- Files changed
- Tests passed
- Security flags
- Approvals pending
- Summary of changes

---

## 16. Responsive Desktop Behavior

Legion IDE is a desktop app, but it should adapt gracefully to smaller laptop windows.

### Wide Layout

- Full left sidebar
- Full right Directive Console
- Main canvas spacious
- Bottom panel expandable

### Medium Layout

- Left sidebar narrows
- Right console can collapse into drawer
- Kanban board scrolls horizontally
- Bottom panel becomes shorter

### Compact Desktop Layout

- Left sidebar collapses to icon rail
- Directive Console becomes slide-over
- Autonomy Scale remains visible
- Main editor/board remains primary
- Agent Comm Stream collapses to ticker

---

## 17. Implementation-Oriented CSS Tokens

```css
:root {
  color-scheme: dark;

  --bg-root: #0D0D12;
  --bg-base: #111118;
  --bg-raised: #15151F;
  --bg-elevated: #1A1A24;
  --bg-hover: #20202B;
  --bg-selected: #252535;
  --bg-input: #101018;
  --bg-code: #0B0B10;

  --border-subtle: rgba(255, 255, 255, 0.05);
  --border-default: rgba(255, 255, 255, 0.08);
  --border-strong: rgba(255, 255, 255, 0.14);
  --border-focus: rgba(107, 92, 255, 0.65);

  --text-primary: #F4F4F6;
  --text-secondary: #B6B7C3;
  --text-muted: #7E8190;
  --text-disabled: #555868;
  --text-inverse: #09090D;

  --accent-cyan: #39D7FF;
  --accent-blue: #4B8CFF;
  --accent-violet: #8B5CFF;
  --accent-purple: #B16CFF;
  --accent-amber: #FFCC66;
  --accent-green: #4ADE80;
  --accent-red: #FF5C7A;

  --status-idle: #6B6E7D;
  --status-thinking: #F5B85B;
  --status-planning: #FFCC66;
  --status-writing: #4B8CFF;
  --status-reviewing: #8B5CFF;
  --status-testing: #39D7FF;
  --status-passed: #4ADE80;
  --status-failed: #FF5C7A;
  --status-waiting: #FFB86B;

  --font-ui: Inter, SF Pro Display, SF Pro Text, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  --font-code: "JetBrains Mono", "Berkeley Mono", "Geist Mono", "SF Mono", Consolas, monospace;

  --text-xs: 11px;
  --text-sm: 12px;
  --text-md: 13px;
  --text-lg: 15px;
  --text-xl: 18px;
  --text-2xl: 24px;

  --line-xs: 16px;
  --line-sm: 18px;
  --line-md: 20px;
  --line-lg: 22px;
  --line-xl: 26px;
  --line-2xl: 32px;

  --space-1: 4px;
  --space-2: 8px;
  --space-3: 12px;
  --space-4: 16px;
  --space-5: 20px;
  --space-6: 24px;
  --space-8: 32px;
  --space-10: 40px;
  --space-12: 48px;

  --radius-sm: 6px;
  --radius-md: 8px;
  --radius-lg: 12px;
  --radius-xl: 16px;
  --radius-pill: 999px;

  --shadow-soft: 0 8px 30px rgba(0, 0, 0, 0.28);
  --shadow-panel: 0 16px 60px rgba(0, 0, 0, 0.36);
  --glow-cyan: 0 0 24px rgba(57, 215, 255, 0.22);
  --glow-violet: 0 0 28px rgba(139, 92, 255, 0.26);
  --glow-fleet: 0 0 32px rgba(177, 108, 255, 0.30), 0 0 18px rgba(57, 215, 255, 0.16);

  --ease-standard: cubic-bezier(0.2, 0.8, 0.2, 1);
  --ease-emphasized: cubic-bezier(0.16, 1, 0.3, 1);
}
```

---

## 18. Suggested Component Inventory

Build the product around these reusable components:

```text
AppShell
TopCommandBar
AutonomyScale
WorkspaceStatus
ResourceMonitor
LeftSidebar
FileExplorer
ActiveFleet
FleetTeamGroup
AgentRow
MainCanvas
CodeEditor
DiffViewer
AgentAnnotation
DirectiveConsole
DirectiveInput
ContextScopeSelector
ApprovalQueue
ApprovalItem
AgentDecisionFeed
RiskMonitor
KanbanBoard
KanbanColumn
TaskCard
TaskInspector
BottomPanel
TerminalOutput
TestRunnerPanel
AgentCommStream
CommandPalette
SettingsPanel
PermissionToggle
OnboardingFlow
ConfirmationModal
Toast
StatusChip
ProgressBar
```

---

## 19. MVP Screen Priority

For an initial prototype, design these first:

1. Core app shell
2. Autonomy Scale
3. Level 1 Manual editor
4. Level 3 Co-Pilot diff review
5. Level 5 Autonomous Fleet board
6. Directive Console
7. Active Fleet sidebar
8. Agent Comm Stream
9. Permission confirmation modal
10. Completed directive summary

---

## 20. Quality Bar Checklist

Before accepting a design, check:

- Does it avoid looking like VS Code?
- Is the Autonomy Scale clearly the central product control?
- Can the user immediately tell what autonomy level they are in?
- Does Level 5 feel like managing an autonomous engineering fleet?
- Are code, diffs, tests, and approvals visible?
- Does the UI communicate safety and control?
- Are agent actions auditable without exposing raw hidden reasoning?
- Is the visual design premium, dark, and restrained?
- Is color used meaningfully rather than decoratively?
- Does the interface still feel usable by expert developers?

---

## 21. One-Sentence Design North Star

> Legion IDE should feel like the first truly native AI development environment: a fast code editor when you want control, a collaborative co-pilot when you want help, and an autonomous engineering command center when you want the fleet to execute.
