import {
  FolderOpen,
  FileCode2,
  FileText,
  ChevronRight,
  ChevronDown,
  Boxes,
  GitBranch,
  CheckCircle2,
  CircleDot,
  GitMerge,
  Workflow,
  Wand2,
  WandSparkles,
  BookOpen,
  TestTube2,
  Lightbulb,
  Check,
  Wrench,
  Server,
  Database,
  ShieldCheck,
  ShieldOff,
} from "lucide-react";
import { MANUAL_TOOLCHAIN, MANUAL_TRUST_BOUNDARY } from "../manualModeProjection";

type Status =
  | "planning"
  | "writing"
  | "reviewing"
  | "testing"
  | "idle";

const STATUS: Record<Status, { color: string; label: string }> = {
  planning: { color: "#FFCC66", label: "Planning" },
  writing: { color: "#4B8CFF", label: "Writing" },
  reviewing: { color: "#8B5CFF", label: "Reviewing" },
  testing: { color: "#39D7FF", label: "Testing" },
  idle: { color: "#6B6E7D", label: "Idle" },
};

const AGENTS: { name: string; model: string; role: string; status: Status; task: string; progress: number }[] = [
  { name: "Claude", model: "opus-4.7", role: "Planner", status: "planning", task: "Decomposing Stripe subscription directive", progress: 38 },
  { name: "GPT-5.5", model: "turbo", role: "Backend", status: "writing", task: "api/billing/checkout.ts", progress: 68 },
  { name: "Gemini", model: "2.5-pro", role: "Reviewer", status: "reviewing", task: "auth/session.ts · 3 nits", progress: 80 },
  { name: "Local", model: "qwen3-32b", role: "QA", status: "testing", task: "billing.test.ts · 6/14", progress: 42 },
];

const WORKSPACE_PACKAGES = [
  { name: "apps/web", status: "active", detail: "Next.js" },
  { name: "apps/api", status: "healthy", detail: "Node :8080" },
  { name: "packages/auth", status: "watched", detail: "12 tests" },
  { name: "packages/billing", status: "changed", detail: "3 files" },
];

const MANUAL_SERVICES = [
  { name: "web", detail: ":3000", status: "running", color: "#4ADE80" },
  { name: "api", detail: ":8080", status: "running", color: "#4ADE80" },
  { name: "postgres", detail: ":5432", status: "healthy", color: "#4B8CFF" },
  { name: "redis", detail: ":6379", status: "healthy", color: "#4B8CFF" },
];

const TOOL_HEALTH_COLOR: Record<string, string> = {
  running: "#4ADE80",
  ready: "#4B8CFF",
  healthy: "#4ADE80",
  idle: "#7E8190",
  degraded: "#FFCC66",
};

function ManualToolchainPanel() {
  return (
    <div className="border-t flex-1 min-h-0 overflow-y-auto" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
      <div className="px-2 pt-2.5 pb-2">
        <div className="px-1 flex items-center justify-between mb-1.5">
          <span className="text-[10px] uppercase tracking-[0.16em] text-white/35 flex items-center gap-1.5">
            <Boxes className="w-3 h-3" style={{ color: "#4B8CFF" }} /> Workspace Packages
          </span>
          <span className="text-[10px] text-white/40 font-mono">{WORKSPACE_PACKAGES.length}</span>
        </div>
        <div className="space-y-1">
          {WORKSPACE_PACKAGES.map((pkg) => (
            <div key={pkg.name} className="h-7 px-1.5 rounded-md flex items-center justify-between bg-white/[0.02] border border-white/[0.04]">
              <span className="font-mono text-[11px] text-white/70 truncate">{pkg.name}</span>
              <span className="text-[10px] text-white/40">{pkg.detail}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="px-2 py-2 border-t" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div className="px-1 flex items-center justify-between mb-1.5">
          <span className="text-[10px] uppercase tracking-[0.16em] text-white/35 flex items-center gap-1.5">
            <Server className="w-3 h-3" style={{ color: "#4ADE80" }} /> Services
          </span>
          <span className="text-[10px] text-white/40">local</span>
        </div>
        <div className="grid grid-cols-2 gap-1">
          {MANUAL_SERVICES.map((service) => (
            <div key={service.name} className="rounded-md border px-2 py-1.5" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
              <div className="flex items-center gap-1.5 text-[11px] text-white/75">
                <span className="w-1.5 h-1.5 rounded-full" style={{ background: service.color }} />
                <span className="truncate">{service.name}</span>
              </div>
              <div className="mt-0.5 flex items-center justify-between text-[9.5px] font-mono">
                <span className="text-white/35">{service.detail}</span>
                <span style={{ color: service.color }}>{service.status}</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="px-2 py-2 border-t" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div className="px-1 flex items-center justify-between mb-1.5">
          <span className="text-[10px] uppercase tracking-[0.16em] text-white/35 flex items-center gap-1.5">
            <Wrench className="w-3 h-3" style={{ color: "#4B8CFF" }} /> Local Toolchain
          </span>
          <span className="text-[10px] text-white/40">deterministic</span>
        </div>
        <div className="space-y-1">
          {MANUAL_TOOLCHAIN.slice(0, 8).map((tool) => (
            <div key={tool.id} className="rounded-md border px-2 py-1.5" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
              <div className="flex items-center gap-2">
                <span className="w-1.5 h-1.5 rounded-full shrink-0" style={{ background: TOOL_HEALTH_COLOR[tool.health] }} />
                <span className="text-[11.5px] text-white/80 truncate">{tool.label}</span>
                <span className="ml-auto text-[9.5px] uppercase tracking-wide" style={{ color: TOOL_HEALTH_COLOR[tool.health] }}>
                  {tool.health}
                </span>
              </div>
              <div className="mt-0.5 flex items-center justify-between text-[9.5px] text-white/35">
                <span>{tool.providerKind}</span>
                <span className="font-mono">{tool.freshness}</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="px-2 py-2 border-t" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div className="px-1 text-[10px] uppercase tracking-[0.16em] text-white/35 flex items-center gap-1.5 mb-1.5">
          <ShieldCheck className="w-3 h-3" style={{ color: "#7E8190" }} /> Manual Trust Boundary
        </div>
        <div className="flex flex-wrap gap-1 px-1">
          {MANUAL_TRUST_BOUNDARY.map((item) => (
            <span key={item} className="px-1.5 py-[2px] rounded text-[9.5px] text-white/55 bg-white/[0.03] border border-white/[0.06]">
              {item}
            </span>
          ))}
        </div>
      </div>
    </div>
  );
}

function FileRow({
  icon,
  name,
  indent = 0,
  active = false,
  badge,
  muted,
  open,
}: any) {
  return (
    <div
      className={`group flex items-center gap-1.5 h-[24px] rounded-md text-[12px] cursor-default ${
        active ? "text-white" : "text-white/55 hover:text-white/85"
      }`}
      style={{
        paddingLeft: 8 + indent * 12,
        background: active ? "#252535" : "transparent",
      }}
    >
      {icon}
      <span className={`truncate ${muted ? "text-white/40" : ""}`}>{name}</span>
      {badge && (
        <span className="ml-auto pr-2 text-[10px]" style={{ color: badge === "M" ? "#FFCC66" : "#39D7FF" }}>
          {badge}
        </span>
      )}
    </div>
  );
}

export function LeftSidebar({ level = 3 }: { level?: number }) {
  const manual = level === 1;
  const assisted = level === 2;
  const copilot = level === 3;
  const delegated = level === 4;
  const fleet = level === 5;

  const fleetTeams = [
    {
      name: "Frontend Team",
      agents: [
        { name: "UI", model: "GPT-5.5", role: "Component Architect", status: "writing" as Status, task: "checkout-modal.tsx" },
        { name: "State", model: "Claude", role: "State Manager", status: "planning" as Status, task: "subscription-store.ts" },
      ]
    },
    {
      name: "Backend Team",
      agents: [
        { name: "API", model: "GPT-5.5", role: "Endpoint Engineer", status: "writing" as Status, task: "routes/stripe.ts" },
        { name: "DB", model: "Local", role: "Schema Manager", status: "reviewing" as Status, task: "migrations/04_billing.sql" },
      ]
    },
    {
      name: "QA Team",
      agents: [
        { name: "Test", model: "Gemini", role: "Test Engineer", status: "testing" as Status, task: "billing-flow.spec.ts" },
      ]
    },
    {
      name: "Review Team",
      agents: [
        { name: "Sec", model: "Claude", role: "Security Auditor", status: "reviewing" as Status, task: "webhook-signature.ts" },
      ]
    },
    {
      name: "DevOps Team",
      agents: [
        { name: "Ops", model: "Local", role: "Deployment", status: "idle" as Status, task: "Standby" },
      ]
    }
  ];

  const agents = assisted
    ? ([
        { name: "GPT-5.5", model: "turbo", role: "Assistant", status: "writing" as Status, task: "Assisting", progress: 0 },
        { name: "Claude", model: "opus-4.7", role: "Assistant", status: "idle" as Status, task: "Standby", progress: 0 },
      ])
    : copilot
    ? ([
        { name: "GPT-5.5", model: "turbo", role: "Co-Pilot", status: "writing" as Status, task: "Pairing on auth/middleware.ts", progress: 62 },
        { name: "Claude", model: "opus-4.7", role: "Architect", status: "reviewing" as Status, task: "Architecture review", progress: 45 },
        { name: "Local", model: "qwen3-32b", role: "Search", status: "testing" as Status, task: "Fast symbol search", progress: 90 },
      ])
    : delegated
    ? ([
        { name: "Frontend", model: "gpt-5.5", role: "Frontend Agent", status: "writing" as Status, task: "Implementing billing UI", progress: 72 },
        { name: "Backend", model: "claude-opus", role: "Backend Agent", status: "writing" as Status, task: "Updating API routes", progress: 54 },
        { name: "QA", model: "qwen3-32b", role: "QA Agent", status: "testing" as Status, task: "Generating tests · 8/14", progress: 58 },
        { name: "Review", model: "gemini-2.5", role: "Review Agent", status: "reviewing" as Status, task: "Inspecting diffs · 3 flags", progress: 40 },
      ])
    : AGENTS;
  return (
    <div
      className="w-[272px] shrink-0 h-full flex flex-col border-r"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      {/* Project header */}
      <div className="h-9 shrink-0 flex items-center justify-between px-3 border-b" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <span className="text-[10px] uppercase tracking-[0.16em] text-white/35">Project</span>
        <div className="flex items-center gap-1.5 text-[10.5px] text-white/45">
          <GitBranch className="w-3 h-3" />
          <span className="font-mono">stripe-subs</span>
        </div>
      </div>

      {/* File tree */}
      {fleet ? (
        <div className="border-b px-1.5 py-2" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
          <div className="flex items-center gap-1.5 h-6 rounded-md text-[12px] text-white/55 hover:text-white/85 px-1.5 cursor-pointer">
            <ChevronRight className="w-3 h-3 text-white/40" />
            <FolderOpen className="w-3.5 h-3.5" />
            <span>nebula-commerce</span>
          </div>
          <div className="mt-2 px-1.5">
            <span className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1.5 block">Context Packs</span>
            <div className="space-y-1">
              {["Auth system", "Billing model", "API routes", "Test suite", "Deployment config"].map((pack) => (
                <div key={pack} className="flex items-center gap-2 h-6 px-1.5 rounded-md text-[11px] text-white/65 hover:bg-white/[0.03] cursor-pointer">
                  <Boxes className="w-3.5 h-3.5 text-white/40" />
                  <span>{pack}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      ) : (
        <div
          className="px-1.5 pt-2 pb-2 overflow-y-auto flex-shrink-0"
          style={{
            maxHeight: manual ? "38%" : assisted ? "56%" : copilot ? "32%" : delegated ? "26%" : "44%",
            flex: "0 0 auto",
          }}
        >
          <div className="space-y-[1px]">
            <FileRow icon={<ChevronDown className="w-3 h-3 text-white/40" />} name="nebula-commerce" />
            <FileRow icon={<ChevronDown className="w-3 h-3 text-white/40" />} name="src" indent={1} />
            <FileRow icon={<ChevronDown className="w-3 h-3 text-white/40" />} name="app" indent={2} />
            <FileRow icon={<FileCode2 className="w-3.5 h-3.5 text-white/45" />} name="layout.tsx" indent={3} />
            <FileRow icon={<FileCode2 className="w-3.5 h-3.5 text-white/45" />} name="page.tsx" indent={3} />
            <FileRow icon={<ChevronDown className="w-3 h-3 text-white/40" />} name="api" indent={2} />
            <FileRow icon={<ChevronDown className="w-3 h-3 text-white/40" />} name="billing" indent={3} />
            <FileRow icon={<FileCode2 className="w-3.5 h-3.5 text-white/45" />} name="checkout.ts" indent={4} active badge="M" />
            <FileRow icon={<FileCode2 className="w-3.5 h-3.5 text-white/45" />} name="webhook.ts" indent={4} badge="●" />
            <FileRow icon={<ChevronRight className="w-3 h-3 text-white/40" />} name="auth" indent={2} />
            <FileRow icon={<ChevronRight className="w-3 h-3 text-white/40" />} name="components" indent={2} />
            <FileRow icon={<ChevronRight className="w-3 h-3 text-white/40" />} name="tests" indent={2} />
            <FileRow icon={<FileText className="w-3.5 h-3.5 text-white/40" />} name="package.json" indent={1} muted />
            <FileRow icon={<FileText className="w-3.5 h-3.5 text-white/40" />} name="tsconfig.json" indent={1} muted />
            <FileRow icon={<FileText className="w-3.5 h-3.5 text-white/40" />} name="README.md" indent={1} muted />
          </div>
        </div>
      )}

      {/* Session Context (Level 3 only) */}
      {copilot && (
        <div className="border-t px-2 pt-2.5 pb-3" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
          <div className="px-1 flex items-center justify-between mb-1.5">
            <span className="text-[10px] uppercase tracking-[0.16em] text-white/35 flex items-center gap-1.5">
              <Workflow className="w-3 h-3" style={{ color: "#4B8CFF" }} /> Session Context
            </span>
            <span className="text-[10px] text-white/40 font-mono">live</span>
          </div>
          <div className="space-y-2 px-1">
            <div>
              <div className="text-[9.5px] uppercase tracking-[0.16em] text-white/30">Current Task</div>
              <div className="text-[11.5px] text-white/80 leading-snug">Add token expiry validation to auth middleware</div>
            </div>
            <div>
              <div className="text-[9.5px] uppercase tracking-[0.16em] text-white/30">Selected Files</div>
              <div className="space-y-0.5 mt-0.5">
                {["auth/middleware.ts", "models/user.ts", "tests/auth.test.ts"].map((f) => (
                  <div key={f} className="flex items-center gap-1.5 text-[11px] text-white/65 font-mono">
                    <FileCode2 className="w-3 h-3 text-white/40" />
                    <span className="truncate">{f}</span>
                  </div>
                ))}
              </div>
            </div>
            <div>
              <div className="text-[9.5px] uppercase tracking-[0.16em] text-white/30">Relevant Symbols</div>
              <div className="flex flex-wrap gap-1 mt-0.5">
                {["verifyToken", "isExpired", "User.role", "Session"].map((s) => (
                  <span
                    key={s}
                    className="px-1.5 py-[2px] rounded text-[10px] font-mono"
                    style={{ background: "rgba(75,140,255,0.10)", color: "#A8C3FF", border: "1px solid rgba(75,140,255,0.22)" }}
                  >
                    {s}
                  </span>
                ))}
              </div>
            </div>
            <div>
              <div className="text-[9.5px] uppercase tracking-[0.16em] text-white/30">Related Tests</div>
              <div className="text-[11px] text-white/65 font-mono">auth.test.ts <span className="text-white/35">· 12 cases</span></div>
            </div>
          </div>
        </div>
      )}

      {/* AI Assistance (Level 2 only) */}
      {assisted && (
        <div
          className="border-t px-2 pt-2.5 pb-3"
          style={{ borderColor: "rgba(255,255,255,0.05)" }}
        >
          <div className="px-1 flex items-center justify-between mb-1.5">
            <span className="text-[10px] uppercase tracking-[0.16em] text-white/35 flex items-center gap-1.5">
              <WandSparkles className="w-3 h-3" style={{ color: "#39D7FF" }} /> AI Assistance
            </span>
            <span className="text-[10px] text-white/40 font-mono">5 on</span>
          </div>
          <div className="space-y-0.5">
            {[
              { icon: Wand2, label: "Inline completions", on: true },
              { icon: Lightbulb, label: "Quick fixes", on: true },
              { icon: BookOpen, label: "Explain selection", on: true },
              { icon: FileText, label: "Generate docs", on: true },
              { icon: TestTube2, label: "Test suggestions", on: true },
            ].map((f) => (
              <div
                key={f.label}
                className="flex items-center justify-between h-6 px-1.5 rounded-md text-[11.5px] text-white/65 hover:bg-white/[0.03]"
              >
                <div className="flex items-center gap-1.5">
                  <f.icon className="w-3 h-3 text-white/45" />
                  <span>{f.label}</span>
                </div>
                <span
                  className="w-[22px] h-3 rounded-full flex items-center px-[2px]"
                  style={{
                    background: f.on ? "rgba(57,215,255,0.25)" : "rgba(255,255,255,0.06)",
                    border: `1px solid ${f.on ? "rgba(57,215,255,0.5)" : "rgba(255,255,255,0.1)"}`,
                    justifyContent: f.on ? "flex-end" : "flex-start",
                  }}
                >
                  <span
                    className="w-2 h-2 rounded-full"
                    style={{ background: f.on ? "#39D7FF" : "#7E8190" }}
                  />
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Deterministic local tooling in Manual; fleet UI starts at higher autonomy. */}
      {manual ? (
        <ManualToolchainPanel />
      ) : fleet ? (
        <div
          className="border-t pt-2.5 px-1.5 flex-1 overflow-y-auto"
          style={{ borderColor: "rgba(255,255,255,0.05)" }}
        >
          <div className="px-2 flex items-center justify-between mb-2">
            <span className="text-[10px] uppercase tracking-[0.16em] text-[#B16CFF]">
              Active Fleet
            </span>
            <span className="text-[10px] text-white/40">9 active</span>
          </div>
          
          <div className="space-y-3 px-1 pb-3">
            {fleetTeams.map((team) => (
              <div key={team.name}>
                <div className="text-[10.5px] text-white/55 mb-1.5 pl-1">{team.name}</div>
                <div className="space-y-1">
                  {team.agents.map((a) => {
                    const s = STATUS[a.status];
                    return (
                      <div
                        key={a.name}
                        className="group p-2 rounded-lg border transition"
                        style={{
                          background: "#15151F",
                          borderColor: "rgba(255,255,255,0.06)",
                        }}
                      >
                        <div className="flex flex-col gap-1.5">
                          <div className="flex items-center justify-between">
                            <div className="flex items-center gap-1.5">
                              <div className="relative">
                                <div
                                  className="w-5 h-5 rounded grid place-items-center text-[9px] font-mono text-white/75 border"
                                  style={{ background: "#1A1A24", borderColor: "rgba(255,255,255,0.08)" }}
                                >
                                  {a.name[0]}
                                </div>
                                <span
                                  className="absolute -bottom-0.5 -right-0.5 w-1.5 h-1.5 rounded-full"
                                  style={{ background: s.color }}
                                />
                                {s.color !== "#6B6E7D" && (
                                  <span
                                    className="absolute -bottom-0.5 -right-0.5 w-1.5 h-1.5 rounded-full animate-ping opacity-60"
                                    style={{ background: s.color }}
                                  />
                                )}
                              </div>
                              <span className="text-[11px] text-white/90 font-medium">{a.role}</span>
                            </div>
                            <span className="px-1 py-0.5 rounded text-[8.5px] font-mono text-white/40 bg-white/[0.04]">
                              {a.model}
                            </span>
                          </div>
                          <div className="flex items-center gap-1.5 text-[10px] pl-1">
                            <span style={{ color: s.color }}>{s.label}</span>
                            <span className="text-white/30">·</span>
                            <span className="text-white/55 truncate">{a.task}</span>
                          </div>
                        </div>
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        </div>
      ) : (
      <div
        className="border-t pt-2.5 px-1.5 flex-1 overflow-y-auto"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="px-2 flex items-center justify-between mb-2">
          <span className="text-[10px] uppercase tracking-[0.16em] text-white/35">
            {assisted ? "Assistants" : "Active Fleet"}
          </span>
          <span className="text-[10px] text-white/40">{agents.length} {assisted ? "loaded" : "active"}</span>
        </div>

        <div className="space-y-1 px-1">
          {agents.map((a) => {
            const s = STATUS[a.status];
            return (
              <div
                key={a.name}
                className="group p-2 rounded-lg border transition"
                style={{
                  background: "#15151F",
                  borderColor: "rgba(255,255,255,0.06)",
                }}
              >
                <div className="flex items-center gap-2">
                  <div className="relative">
                    <div
                      className="w-6 h-6 rounded-md grid place-items-center text-[10px] font-mono text-white/75 border"
                      style={{ background: "#1A1A24", borderColor: "rgba(255,255,255,0.08)" }}
                    >
                      {a.name[0]}
                    </div>
                    <span
                      className="absolute -bottom-0.5 -right-0.5 w-1.5 h-1.5 rounded-full"
                      style={{ background: s.color }}
                    />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      <span className="text-[12px] text-white/90 font-medium tracking-tight">{a.name}</span>
                      <span className="text-[10px] text-white/30 font-mono">{a.model}</span>
                    </div>
                    <div className="flex items-center gap-1.5 text-[10.5px]">
                      <span style={{ color: s.color }}>{s.label}</span>
                      <span className="text-white/30">·</span>
                      <span className="text-white/55 truncate">{a.task}</span>
                    </div>
                  </div>
                </div>
                <div className="mt-1.5 h-[2px] w-full rounded-full overflow-hidden" style={{ background: "rgba(255,255,255,0.05)" }}>
                  <div className="h-full" style={{ width: `${a.progress}%`, background: s.color, opacity: 0.75 }} />
                </div>
              </div>
            );
          })}
        </div>
      </div>
      )}

      {/* Environment / status footer */}
      <div
        className="border-t px-2.5 py-2 text-[10.5px] space-y-1"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center justify-between text-white/55">
          <span className="flex items-center gap-1.5">
            <CircleDot className="w-3 h-3" style={{ color: "#FFCC66" }} /> git
          </span>
          <span className="text-white/70 font-mono">7 changes</span>
        </div>
        <div className="flex items-center justify-between text-white/55">
          <span className="flex items-center gap-1.5">
            <CheckCircle2 className="w-3 h-3" style={{ color: "#4ADE80" }} /> tests
          </span>
          <span className="text-white/70 font-mono">412 / 412</span>
        </div>
        <div className="flex items-center justify-between text-white/55">
          <span className="flex items-center gap-1.5">
            <GitMerge className="w-3 h-3" style={{ color: "#8B5CFF" }} /> repo
          </span>
          <span className="text-white/70 font-mono truncate">github · nebula</span>
        </div>
        <div className="flex items-center justify-between text-white/55">
          <span className="flex items-center gap-1.5">
            {manual ? (
              <ShieldOff className="w-3 h-3" style={{ color: "#7E8190" }} />
            ) : (
              <Workflow className="w-3 h-3" style={{ color: "#39D7FF" }} />
            )} {manual ? "AI" : "workflows"}
          </span>
          <span className="text-white/70 font-mono">{manual ? "disabled" : "2 active"}</span>
        </div>
      </div>
    </div>
  );
}
