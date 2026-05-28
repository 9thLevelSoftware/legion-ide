export type ManualToolHealth = "running" | "ready" | "idle" | "healthy" | "degraded";

export type ManualProviderKind =
  | "lsp"
  | "parser"
  | "formatter"
  | "linter"
  | "test"
  | "task"
  | "debug"
  | "terminal"
  | "scm"
  | "container"
  | "service"
  | "database"
  | "api"
  | "profiler"
  | "security"
  | "docs";

export type ManualDiagnostic = {
  severity: "error" | "warning" | "hint";
  source: string;
  target: string;
  message: string;
};

export type ManualToolState = {
  id: string;
  label: string;
  providerKind: ManualProviderKind;
  health: ManualToolHealth;
  freshness: string;
  workspaceScope: string;
  capabilities: string[];
  diagnostics: ManualDiagnostic[];
  allowedCommands: string[];
};

export type ManualCommandTarget = {
  id: string;
  kind:
    | "file"
    | "symbol"
    | "task"
    | "test"
    | "debug"
    | "service"
    | "branch"
    | "worktree"
    | "api"
    | "database"
    | "docs"
    | "settings";
  label: string;
  scope: string;
  status: string;
};

export const MANUAL_TRUST_BOUNDARY = [
  "AI Disabled",
  "Local Tools Only",
  "No Model Calls",
  "No Agent Context",
  "No Autonomous Writes",
];

export const MANUAL_TOOLCHAIN: ManualToolState[] = [
  {
    id: "typescript-lsp",
    label: "TypeScript LSP",
    providerKind: "lsp",
    health: "running",
    freshness: "live",
    workspaceScope: "apps/web",
    capabilities: ["completion", "hover", "references", "rename", "code actions"],
    diagnostics: [
      {
        severity: "warning",
        source: "tsserver",
        target: "api/auth.ts:23",
        message: "payload.role is unknown until claim narrowing runs.",
      },
    ],
    allowedCommands: ["Go to Definition", "Find References", "Rename Symbol", "Quick Fix"],
  },
  {
    id: "tree-sitter",
    label: "Tree-sitter Syntax Cache",
    providerKind: "parser",
    health: "running",
    freshness: "18 ms",
    workspaceScope: "open editors",
    capabilities: ["folding", "selection ranges", "symbol outline", "scope queries"],
    diagnostics: [],
    allowedCommands: ["Expand Selection", "Fold Block", "Open Symbol"],
  },
  {
    id: "eslint",
    label: "ESLint",
    providerKind: "linter",
    health: "ready",
    freshness: "2s",
    workspaceScope: "workspace",
    capabilities: ["diagnostics", "fix all", "suppression review"],
    diagnostics: [
      {
        severity: "hint",
        source: "eslint",
        target: "api/auth.ts:15",
        message: "Prefer explicit cookie maxAge for long-lived sessions.",
      },
    ],
    allowedCommands: ["Fix All", "Open Rule", "Suppress with Reason"],
  },
  {
    id: "prettier",
    label: "Prettier",
    providerKind: "formatter",
    health: "ready",
    freshness: "configured",
    workspaceScope: "workspace",
    capabilities: ["format document", "format changed ranges"],
    diagnostics: [],
    allowedCommands: ["Format Document", "Format Changed Ranges"],
  },
  {
    id: "vitest",
    label: "Vitest Watcher",
    providerKind: "test",
    health: "idle",
    freshness: "412 passed",
    workspaceScope: "packages/auth",
    capabilities: ["discover tests", "run nearest", "debug nearest", "coverage"],
    diagnostics: [],
    allowedCommands: ["Run Nearest Test", "Debug Nearest Test", "Toggle Coverage"],
  },
  {
    id: "task-runner",
    label: "Task Runner",
    providerKind: "task",
    health: "running",
    freshness: "pnpm dev",
    workspaceScope: "workspace",
    capabilities: ["problem matchers", "watch tasks", "rerun last task"],
    diagnostics: [],
    allowedCommands: ["Run Task", "Watch Task", "Open Output"],
  },
  {
    id: "dap",
    label: "DAP Debugger",
    providerKind: "debug",
    health: "ready",
    freshness: "node attach ready",
    workspaceScope: "local runtime",
    capabilities: ["breakpoints", "watches", "call stack", "debug console"],
    diagnostics: [],
    allowedCommands: ["Start Debugging", "Attach to Process", "Toggle Breakpoint"],
  },
  {
    id: "git",
    label: "Git Worktree",
    providerKind: "scm",
    health: "ready",
    freshness: "7 changes",
    workspaceScope: "feature/stripe-subscriptions",
    capabilities: ["partial staging", "blame", "history", "conflict editor"],
    diagnostics: [],
    allowedCommands: ["Stage Hunk", "Open Blame", "Switch Worktree"],
  },
  {
    id: "docker",
    label: "Docker Compose",
    providerKind: "container",
    health: "healthy",
    freshness: "4 services",
    workspaceScope: "devcontainer",
    capabilities: ["service health", "logs", "port forwarding"],
    diagnostics: [],
    allowedCommands: ["Restart Service", "Open Logs", "Forward Port"],
  },
  {
    id: "postgres",
    label: "Postgres",
    providerKind: "database",
    health: "healthy",
    freshness: "local",
    workspaceScope: "postgres://local",
    capabilities: ["schema explorer", "SQL", "explain plan", "migrations"],
    diagnostics: [],
    allowedCommands: ["Open Table", "Run Query", "Explain Query"],
  },
  {
    id: "http-client",
    label: ".http Client",
    providerKind: "api",
    health: "ready",
    freshness: "3 environments",
    workspaceScope: "local API",
    capabilities: ["REST", "GraphQL", "OpenAPI", "response diff"],
    diagnostics: [],
    allowedCommands: ["Send Request", "Switch Environment", "Open Response"],
  },
  {
    id: "supply-chain",
    label: "Supply Chain Scan",
    providerKind: "security",
    health: "ready",
    freshness: "lockfile clean",
    workspaceScope: "workspace",
    capabilities: ["audit", "secret scan", "license review", "SBOM"],
    diagnostics: [],
    allowedCommands: ["Run Audit", "Open SBOM", "Review Licenses"],
  },
];

export const MANUAL_COMMAND_TARGETS: ManualCommandTarget[] = [
  { id: "task-dev", kind: "task", label: "Run dev server", scope: "pnpm dev", status: "running" },
  { id: "test-auth", kind: "test", label: "Run auth tests", scope: "auth.test.ts", status: "412 passing" },
  { id: "debug-node", kind: "debug", label: "Attach Node debugger", scope: ":9229", status: "ready" },
  { id: "svc-api", kind: "service", label: "Restart api service", scope: ":8080", status: "healthy" },
  { id: "git-hunk", kind: "branch", label: "Stage selected hunk", scope: "checkout.ts", status: "available" },
  { id: "api-checkout", kind: "api", label: "POST /billing/checkout", scope: "billing.http", status: "ready" },
  { id: "db-subs", kind: "database", label: "Open subscriptions table", scope: "public.subscriptions", status: "local" },
  { id: "docs-jose", kind: "docs", label: "Open jose package docs", scope: "current import", status: "cached" },
];
