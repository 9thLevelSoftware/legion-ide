import {
  Sparkles,
  Paperclip,
  AtSign,
  CornerDownLeft,
  Check,
  AlertTriangle,
  Clock,
  ChevronDown,
  Shield,
  Scan,
  X,
  Wand2,
  Lightbulb,
  TestTube2,
  BookOpen,
  MessageSquare,
  ArrowRight,
  GitBranch,
  Gauge,
  Database,
  Braces,
  Wrench,
  ShieldOff,
} from "lucide-react";
import { MANUAL_COMMAND_TARGETS, MANUAL_TOOLCHAIN, MANUAL_TRUST_BOUNDARY } from "../manualModeProjection";

const APPROVALS = [
  {
    title: "Apply migration to user_subscriptions",
    agent: "GPT-5.5 · Backend",
    risk: "Medium",
    riskColor: "#FFCC66",
    files: 3,
  },
  {
    title: "Install dependency: stripe@14.0.0",
    agent: "Claude · Planner",
    risk: "Low",
    riskColor: "#4ADE80",
    files: 1,
  },
];

const DECISIONS = [
  { who: "Claude", body: "Selected Stripe checkout sessions over payment links for richer metadata.", time: "12:04" },
  { who: "Gemini", body: "Flagged missing webhook idempotency guard in api/billing/webhook.ts.", time: "12:06" },
  { who: "Local", body: "Generated 6 subscription lifecycle regression tests.", time: "12:08" },
];

const ACTIVITY = [
  { tag: "PLAN", color: "#FFCC66", body: "Claude → Backend: assigned checkout session endpoint", time: "12:04:11" },
  { tag: "WRITE", color: "#4B8CFF", body: "GPT-5.5: added Stripe SDK integration", time: "12:04:18" },
  { tag: "TEST", color: "#39D7FF", body: "Local: generated 6 subscription lifecycle tests", time: "12:04:31" },
  { tag: "REVIEW", color: "#8B5CFF", body: "Gemini: flagged missing webhook idempotency guard", time: "12:04:44" },
];

function ManualContextInspector({ level }: { level: number }) {
  const diagnostics = MANUAL_TOOLCHAIN.flatMap((tool) => tool.diagnostics);
  const featuredTools = MANUAL_TOOLCHAIN.filter((tool) =>
    ["typescript-lsp", "tree-sitter", "vitest", "dap", "git", "postgres", "supply-chain"].includes(tool.id)
  );
  const tabs = [
    "Symbols",
    "Problems",
    "Tests",
    "Git",
    "Debug",
    "Docs",
    "Security",
    "Performance",
  ];

  return (
    <div
      className="w-[340px] shrink-0 h-full flex flex-col border-l"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      <div
        className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-2">
          <Wrench className="w-3.5 h-3.5" style={{ color: "#4B8CFF" }} />
          <span className="text-[12px] text-white/85 font-medium tracking-tight">Manual Tooling</span>
        </div>
        <span
          className="px-1.5 py-[2px] rounded text-[10px] font-mono"
          style={{ color: "#A8C3FF", background: "rgba(75,140,255,0.10)", border: "1px solid rgba(75,140,255,0.24)" }}
        >
          L{level} · AI OFF
        </span>
      </div>

      <div className="h-8 shrink-0 flex items-center gap-1 px-2 border-b overflow-x-auto" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        {tabs.map((tab, index) => (
          <span
            key={tab}
            className="h-6 px-2 inline-flex items-center rounded-md text-[10.5px] whitespace-nowrap"
            style={{
              color: index === 0 ? "#F4F4F6" : "rgba(255,255,255,0.50)",
              background: index === 0 ? "rgba(255,255,255,0.05)" : "transparent",
            }}
          >
            {tab}
          </span>
        ))}
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-3 space-y-4 text-[11.5px]">
        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Trust Boundary</div>
          <div className="grid grid-cols-2 gap-1">
            {MANUAL_TRUST_BOUNDARY.map((item) => (
              <div key={item} className="px-2 py-1.5 rounded-md border text-[10.5px] text-white/65 flex items-center gap-1.5" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
                <ShieldOff className="w-3 h-3 text-white/35" />
                <span className="truncate">{item}</span>
              </div>
            ))}
          </div>
        </section>

        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Current File</div>
          <div className="text-white/80 font-mono">auth.ts</div>
          <div className="text-white/40 text-[10.5px]">src/api/auth · LSP live · Tree-sitter 18 ms</div>
        </section>

        <section>
          <div className="flex items-center justify-between mb-1.5">
            <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">Symbols</div>
            <span className="text-[10px] text-white/40">LSP + parser</span>
          </div>
          <div className="space-y-1 rounded-md border p-2" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
            {[
              { kind: "fn", name: "issueSession", line: 9, color: "#FFCC66" },
              { kind: "fn", name: "readSession", line: 19, color: "#FFCC66" },
              { kind: "const", name: "SECRET", line: 5, color: "#39D7FF" },
              { kind: "const", name: "COOKIE", line: 6, color: "#39D7FF" },
            ].map((s) => (
              <div key={s.name} className="flex items-center justify-between text-white/65">
                <div className="flex items-center gap-2 min-w-0">
                  <span className="font-mono text-[9.5px] w-7" style={{ color: s.color }}>{s.kind}</span>
                  <span className="font-mono truncate">{s.name}</span>
                </div>
                <span className="text-white/30 font-mono text-[10px]">:{s.line}</span>
              </div>
            ))}
          </div>
        </section>

        <section>
          <div className="flex items-center justify-between mb-1.5">
            <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">Problems</div>
            <span className="text-[10px] text-white/40">{diagnostics.length} mapped</span>
          </div>
          <div className="space-y-1.5">
            {diagnostics.map((problem) => (
              <div key={`${problem.source}-${problem.target}`} className="rounded-md border p-2" style={{ background: "#15151F", borderColor: problem.severity === "warning" ? "rgba(255,204,102,0.20)" : "rgba(75,140,255,0.16)" }}>
                <div className="flex items-center gap-1.5 text-[10px]">
                  <AlertTriangle className="w-3 h-3" style={{ color: problem.severity === "warning" ? "#FFCC66" : "#4B8CFF" }} />
                  <span className="uppercase tracking-wide" style={{ color: problem.severity === "warning" ? "#FFCC66" : "#4B8CFF" }}>{problem.severity}</span>
                  <span className="text-white/30">·</span>
                  <span className="font-mono text-white/45">{problem.target}</span>
                </div>
                <div className="mt-1 text-white/75 leading-snug">{problem.message}</div>
              </div>
            ))}
          </div>
        </section>

        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Test Explorer</div>
          <div className="rounded-md border p-2 space-y-1" style={{ background: "#15151F", borderColor: "rgba(74,222,128,0.14)" }}>
            {[
              ["auth/session.test.ts", "19 passed", "#4ADE80"],
              ["billing/webhook.test.ts", "8 passed", "#4ADE80"],
              ["coverage", "91.4%", "#4B8CFF"],
            ].map(([name, status, color]) => (
              <div key={name} className="flex items-center gap-2 text-[11px]">
                <TestTube2 className="w-3 h-3" style={{ color }} />
                <span className="font-mono text-white/70 truncate">{name}</span>
                <span className="ml-auto font-mono" style={{ color }}>{status}</span>
              </div>
            ))}
          </div>
        </section>

        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Git Cockpit</div>
          <div className="space-y-0.5 text-white/65 rounded-md border p-2" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
            {[
              { f: "auth.ts", t: "M", c: "#FFCC66" },
              { f: "subscriptions.ts", t: "M", c: "#FFCC66" },
              { f: "models/user.ts", t: "A", c: "#4ADE80" },
            ].map((g) => (
              <div key={g.f} className="flex items-center gap-2 font-mono text-[11px]">
                <span className="w-3 text-center" style={{ color: g.c }}>{g.t}</span>
                <span className="truncate text-white/70">{g.f}</span>
              </div>
            ))}
            <div className="pt-1 mt-1 border-t flex items-center gap-1.5 text-[10.5px]" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
              <GitBranch className="w-3 h-3 text-white/35" />
              <span className="text-white/45">worktree</span>
              <span className="ml-auto font-mono text-white/65">feature/stripe-subscriptions</span>
            </div>
          </div>
        </section>

        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Debugger + Tools</div>
          <div className="grid grid-cols-2 gap-1.5">
            {featuredTools.map((tool) => (
              <div key={tool.id} className="rounded-md border p-2" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
                <div className="flex items-center gap-1.5 text-white/75">
                  {tool.providerKind === "database" ? (
                    <Database className="w-3 h-3" style={{ color: "#4B8CFF" }} />
                  ) : tool.providerKind === "profiler" ? (
                    <Gauge className="w-3 h-3" style={{ color: "#FFCC66" }} />
                  ) : tool.providerKind === "parser" ? (
                    <Braces className="w-3 h-3" style={{ color: "#4B8CFF" }} />
                  ) : (
                    <Check className="w-3 h-3" style={{ color: "#4ADE80" }} />
                  )}
                  <span className="truncate">{tool.label}</span>
                </div>
                <div className="mt-1 text-[10px] text-white/40 font-mono">{tool.freshness}</div>
              </div>
            ))}
          </div>
        </section>

        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Command Targets</div>
          <div className="space-y-1">
            {MANUAL_COMMAND_TARGETS.slice(0, 5).map((target) => (
              <div key={target.id} className="h-7 px-2 rounded-md border flex items-center gap-2" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
                <span className="text-[9.5px] uppercase tracking-wide text-white/35 w-14">{target.kind}</span>
                <span className="text-white/70 truncate">{target.label}</span>
                <span className="ml-auto font-mono text-[10px] text-white/40">{target.status}</span>
              </div>
            ))}
          </div>
        </section>
      </div>

      <div className="border-t px-3 py-2" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div className="flex items-center gap-1.5">
          <button
            className="flex-1 h-7 flex items-center justify-center gap-1.5 rounded-md text-[11px] text-white/75 hover:text-white border"
            style={{ background: "rgba(75,140,255,0.10)", borderColor: "rgba(75,140,255,0.22)" }}
          >
            Run Task <span className="ml-1 text-[10px] text-white/35 font-mono">⌘K</span>
          </button>
          <button
            className="flex-1 h-7 flex items-center justify-center gap-1.5 rounded-md text-[11px] text-white/60 hover:text-white border"
            style={{ background: "rgba(255,255,255,0.03)", borderColor: "rgba(255,255,255,0.08)" }}
          >
            Open Problems
          </button>
        </div>
      </div>
    </div>
  );
}

function AssistedPanel({ level }: { level: number }) {
  const ACCENT = "#39D7FF";
  const ACTIONS = [
    { icon: Lightbulb, label: "Suggested Fixes", count: 3, hint: "Null-guard, narrow types, retry", color: "#FFCC66" },
    { icon: BookOpen, label: "Explain This Function", count: null, hint: "createSubscription · 12 lines", color: "#4B8CFF" },
    { icon: TestTube2, label: "Generate Test", count: null, hint: "vitest · cancels active sub", color: "#39D7FF" },
    { icon: Wand2, label: "Refactor Selection", count: null, hint: "extract validateCustomer()", color: "#8B5CFF" },
  ];
  const RECENT = [
    "Added null guard for customerId",
    "Generated docstring for issueSession",
    "Suggested narrowing User['role'] to literal union",
  ];
  return (
    <div
      className="w-[340px] shrink-0 h-full flex flex-col border-l"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      <div
        className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-2">
          <Sparkles className="w-3.5 h-3.5" style={{ color: ACCENT }} />
          <span className="text-[12px] text-white/90 font-medium tracking-tight">Assistant</span>
          <span className="text-[10px] text-white/40">· subscriptions.ts</span>
        </div>
        <span
          className="px-1.5 py-[2px] rounded text-[10px] font-mono"
          style={{ color: "#9EE9FF", background: "rgba(57,215,255,0.10)", border: "1px solid rgba(57,215,255,0.25)" }}
        >
          L{level}
        </span>
      </div>

      {/* Current selection */}
      <div className="px-3 pt-3">
        <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Current Selection</div>
        <div
          className="rounded-lg border p-2.5"
          style={{ background: "#15151F", borderColor: "rgba(75,140,255,0.22)" }}
        >
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[11px] text-white/65 font-mono">subscriptions.ts <span className="text-white/35">:7–11</span></span>
            <span className="text-[10px] text-white/45">5 lines</span>
          </div>
          <pre
            className="font-mono text-[10.5px] leading-[1.55] text-white/65 overflow-hidden whitespace-pre-wrap"
            style={{ background: "#0B0B10", borderRadius: 6, padding: "8px 10px", border: "1px solid rgba(255,255,255,0.05)" }}
          >{`const subscription = await stripe.subscriptions.create({
  customer: customerId,
  items: [{ price: priceId }],
  payment_behavior: "default_incomplete",
});`}</pre>
        </div>
      </div>

      {/* Actions */}
      <div className="px-3 pt-3 space-y-1">
        <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1">Actions</div>
        {ACTIONS.map((a) => (
          <button
            key={a.label}
            className="w-full flex items-center gap-2.5 px-2.5 h-9 rounded-md text-left border"
            style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}
          >
            <a.icon className="w-3.5 h-3.5" style={{ color: a.color }} />
            <div className="min-w-0 flex-1">
              <div className="text-[11.5px] text-white/85 leading-tight">{a.label}</div>
              <div className="text-[10px] text-white/40 truncate">{a.hint}</div>
            </div>
            {a.count !== null && (
              <span
                className="text-[10px] px-1.5 py-[1px] rounded font-mono"
                style={{ color: a.color, background: "rgba(255,255,255,0.04)", border: `1px solid ${a.color}33` }}
              >
                {a.count}
              </span>
            )}
            <ArrowRight className="w-3 h-3 text-white/30" />
          </button>
        ))}
      </div>

      {/* Recent assists */}
      <div className="px-3 pt-3 flex-1 overflow-y-auto">
        <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Recent Assists</div>
        <div className="space-y-1">
          {RECENT.map((r, i) => (
            <div key={i} className="flex items-start gap-1.5 text-[11px] text-white/65">
              <Check className="w-3 h-3 mt-[3px] shrink-0" style={{ color: "#4ADE80" }} />
              <span>{r}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Ask box */}
      <div className="border-t p-3" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div
          className="rounded-lg border focus-within:border-white/20"
          style={{ background: "#101018", borderColor: "rgba(255,255,255,0.08)" }}
        >
          <div className="flex items-center gap-2 px-2.5 py-1.5">
            <MessageSquare className="w-3.5 h-3.5 text-white/40" />
            <input
              className="flex-1 bg-transparent outline-none text-[11.5px] text-white/85 placeholder:text-white/30"
              placeholder="Ask about this file or selection…"
              defaultValue=""
            />
            <span className="text-[10px] text-white/35 font-mono">⌘L</span>
          </div>
        </div>
        <div className="mt-1.5 flex items-center justify-between text-[10px] text-white/40">
          <span>Context: file + selection</span>
          <span className="font-mono">gpt-5.5 · turbo</span>
        </div>
      </div>
    </div>
  );
}

function DelegationConsole({ level }: { level: number }) {
  const APPROVALS_DEL = [
    {
      title: "Backend diff: webhook idempotency guard",
      agent: "Backend Agent",
      agentColor: "#4B8CFF",
      risk: "Medium",
      riskColor: "#FFCC66",
      files: 1,
      meta: "+8 / −1",
    },
    {
      title: "QA-generated subscription lifecycle tests",
      agent: "QA Agent",
      agentColor: "#39D7FF",
      risk: "Low",
      riskColor: "#4ADE80",
      files: 2,
      meta: "8 cases",
    },
    {
      title: "Rollback: provider abstraction in billing/",
      agent: "Review Agent",
      agentColor: "#8B5CFF",
      risk: "High",
      riskColor: "#FF5C7A",
      files: 4,
      meta: "revert 3 commits",
    },
  ];
  const DECISIONS_DEL = [
    { q: "Approve high-risk schema migration on user_billing?", agent: "Backend Agent", time: "now" },
    { q: "Accept Review Agent's rollback recommendation?", agent: "Review Agent", time: "2m" },
  ];
  return (
    <div
      className="w-[380px] shrink-0 h-full flex flex-col border-l"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      <div
        className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-2">
          <Sparkles className="w-3.5 h-3.5" style={{ color: "#8B5CFF" }} />
          <span className="text-[12px] text-white/90 font-medium tracking-tight">Delegation Console</span>
        </div>
        <span
          className="px-1.5 py-[2px] rounded text-[10px] font-mono"
          style={{ color: "#C8B5FF", background: "rgba(139,92,255,0.10)", border: "1px solid rgba(139,92,255,0.25)" }}
        >
          L{level}
        </span>
      </div>

      {/* Delegate input */}
      <div className="p-3">
        <div
          className="rounded-xl border focus-within:border-white/20"
          style={{ background: "#101018", borderColor: "rgba(255,255,255,0.08)" }}
        >
          <div className="px-3 pt-2.5 flex items-center justify-between">
            <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">Delegate Task</span>
            <span className="text-[10px] text-white/40 font-mono">⌘↵</span>
          </div>
          <textarea
            className="w-full bg-transparent resize-none outline-none px-3 py-2 text-[12.5px] text-white/90 placeholder:text-white/30"
            rows={2}
            placeholder="Delegate a task to the fleet…"
          />
          <div
            className="flex items-center justify-between px-2 py-2 border-t"
            style={{ borderColor: "rgba(255,255,255,0.05)" }}
          >
            <div className="flex items-center gap-1 text-white/45">
              <button className="h-6 px-1.5 flex items-center gap-1 rounded-md hover:bg-white/[0.05] text-[10.5px]">
                <Scan className="w-3 h-3" /> Scope
              </button>
              <button className="h-6 px-1.5 flex items-center gap-1 rounded-md hover:bg-white/[0.05] text-[10.5px]">
                <Sparkles className="w-3 h-3" /> Agent
              </button>
              <button className="h-6 px-1.5 flex items-center gap-1 rounded-md hover:bg-white/[0.05] text-[10.5px]">
                <Check className="w-3 h-3" /> Approval
              </button>
              <button className="h-6 px-1.5 flex items-center gap-1 rounded-md hover:bg-white/[0.05] text-[10.5px]">
                <Shield className="w-3 h-3" /> Risk
              </button>
            </div>
            <button
              className="h-6 px-2 flex items-center gap-1.5 rounded-md text-[10.5px] font-semibold"
              style={{ color: "#09090D", background: "linear-gradient(135deg, #39D7FF 0%, #8B5CFF 100%)" }}
            >
              Delegate <CornerDownLeft className="w-3 h-3" />
            </button>
          </div>
        </div>

        {/* Scope chips */}
        <div className="mt-2 flex flex-wrap items-center gap-1 text-[10.5px]">
          <span
            className="h-6 px-2 flex items-center gap-1.5 rounded-md border"
            style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.08)", color: "#B6B7C3" }}
          >
            <Scan className="w-3 h-3" /> module · billing
            <ChevronDown className="w-3 h-3 text-white/40" />
          </span>
          <span
            className="h-6 px-2 flex items-center gap-1.5 rounded-md border"
            style={{ background: "rgba(75,140,255,0.10)", borderColor: "rgba(75,140,255,0.30)", color: "#A8C3FF" }}
          >
            agent · auto-route
          </span>
          <span
            className="h-6 px-2 flex items-center gap-1.5 rounded-md border"
            style={{ background: "rgba(255,184,107,0.08)", borderColor: "rgba(255,184,107,0.28)", color: "#FFB86B" }}
          >
            approval · human gate
          </span>
          <span
            className="h-6 px-2 flex items-center gap-1.5 rounded-md border"
            style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.08)", color: "#B6B7C3" }}
          >
            risk · moderate
          </span>
        </div>
      </div>

      {/* Approval queue */}
      <div className="px-3">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">Approval Queue</span>
          <span
            className="text-[10px] px-1.5 py-[1px] rounded"
            style={{ color: "#FFB86B", background: "rgba(255,184,107,0.08)", border: "1px solid rgba(255,184,107,0.25)" }}
          >
            3 waiting
          </span>
        </div>
        <div className="space-y-1.5">
          {APPROVALS_DEL.map((a, i) => (
            <div
              key={i}
              className="rounded-lg border p-2.5"
              style={{ background: "#15151F", borderColor: "rgba(255,184,107,0.18)" }}
            >
              <div className="text-[12px] text-white/90 leading-snug">{a.title}</div>
              <div className="mt-1 flex items-center gap-1.5 text-[10.5px] text-white/50">
                <span
                  className="w-3.5 h-3.5 grid place-items-center rounded text-[9px] font-mono"
                  style={{ background: `${a.agentColor}1F`, color: a.agentColor, border: `1px solid ${a.agentColor}55` }}
                >
                  {a.agent[0]}
                </span>
                <span>{a.agent}</span>
                <span className="text-white/25">·</span>
                <span style={{ color: a.riskColor }}>{a.risk} risk</span>
                <span className="text-white/25">·</span>
                <span className="font-mono text-white/55">{a.meta}</span>
              </div>
              <div className="mt-2 flex items-center gap-1">
                <button
                  className="h-6 px-2 rounded-md text-[10.5px] font-medium"
                  style={{ background: "rgba(74,222,128,0.12)", color: "#86EFAC", border: "1px solid rgba(74,222,128,0.25)" }}
                >
                  Approve
                </button>
                <button className="h-6 px-2 rounded-md text-[10.5px] text-white/65 bg-white/[0.04] border border-white/[0.08]">
                  Review
                </button>
                <button className="h-6 px-2 rounded-md text-[10.5px] text-rose-300 hover:bg-rose-400/10 border border-transparent">
                  Reject
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Human Decisions Needed */}
      <div className="px-3 mt-3 flex-1 overflow-y-auto">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 flex items-center gap-1.5">
            <AlertTriangle className="w-3 h-3" style={{ color: "#FFB86B" }} /> Human Decisions Needed
          </span>
          <span className="text-[10px] text-white/40">{DECISIONS_DEL.length}</span>
        </div>
        <div className="space-y-1.5">
          {DECISIONS_DEL.map((d, i) => (
            <div
              key={i}
              className="rounded-md border p-2 text-[11.5px]"
              style={{ background: "rgba(255,184,107,0.05)", borderColor: "rgba(255,184,107,0.20)" }}
            >
              <div className="text-white/85 leading-snug">{d.q}</div>
              <div className="mt-0.5 flex items-center justify-between text-[10px] text-white/45">
                <span>{d.agent}</span>
                <span className="font-mono">{d.time}</span>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div
        className="border-t px-3 py-2 flex items-center justify-between text-[10px] text-white/40"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <span>4 agents engaged · L{level}</span>
        <span className="font-mono">auto-route · gpt+claude</span>
      </div>
    </div>
  );
}

function PairSessionPanel({ level }: { level: number }) {
  const PLAN = [
    { n: 1, label: "Inspect existing auth middleware", state: "done" },
    { n: 2, label: "Add token expiry validation", state: "active" },
    { n: 3, label: "Update user model", state: "queued" },
    { n: 4, label: "Generate tests", state: "queued" },
    { n: 5, label: "Run test suite", state: "queued" },
  ];
  const FEEDBACK = [
    { who: "You", body: "Keep the helper colocated with the middleware — don't pull it into utils.", time: "12:06" },
    { who: "GPT-5.5", body: "Acknowledged. Inlining isExpired() in auth/middleware.ts.", time: "12:06", ai: true },
    { who: "You", body: "Use 30s leeway, not 60s.", time: "12:07" },
  ];
  const APPROVALS_PAIR = [
    { title: "Apply diff: auth/middleware.ts (+6 / −1)", agent: "GPT-5.5", risk: "Low", riskColor: "#4ADE80" },
    { title: "Add field: User.tokenExpiresAt", agent: "Claude", risk: "Medium", riskColor: "#FFCC66" },
  ];
  return (
    <div
      className="w-[380px] shrink-0 h-full flex flex-col border-l"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      <div
        className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-2">
          <Sparkles className="w-3.5 h-3.5" style={{ color: "#4B8CFF" }} />
          <span className="text-[12px] text-white/90 font-medium tracking-tight">Pair Session</span>
          <span className="text-[10px] text-white/40">· GPT-5.5</span>
        </div>
        <span
          className="px-1.5 py-[2px] rounded text-[10px] font-mono"
          style={{ color: "#A8C3FF", background: "rgba(75,140,255,0.10)", border: "1px solid rgba(75,140,255,0.25)" }}
        >
          L{level}
        </span>
      </div>

      <div className="flex-1 overflow-y-auto">
        {/* Current Objective */}
        <section className="px-3 pt-3">
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Current Objective</div>
          <div
            className="rounded-lg border p-2.5"
            style={{ background: "#15151F", borderColor: "rgba(75,140,255,0.22)" }}
          >
            <div className="text-[12px] text-white/90 leading-snug">
              Add token expiry validation to auth middleware
            </div>
            <div className="mt-1 flex items-center gap-1.5 text-[10.5px] text-white/45">
              <span className="font-mono">auth/middleware.ts</span>
              <span className="text-white/25">·</span>
              <span>started 12:04</span>
            </div>
          </div>
        </section>

        {/* AI Plan */}
        <section className="px-3 pt-3">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">AI Plan</span>
            <button className="text-[10px] text-white/45 hover:text-white/75">Revise</button>
          </div>
          <div className="space-y-0.5">
            {PLAN.map((s) => {
              const done = s.state === "done";
              const active = s.state === "active";
              const color = done ? "#4ADE80" : active ? "#4B8CFF" : "#6B6E7D";
              return (
                <div
                  key={s.n}
                  className="flex items-center gap-2 h-7 px-2 rounded-md"
                  style={{ background: active ? "rgba(75,140,255,0.08)" : "transparent" }}
                >
                  <span
                    className="w-4 h-4 grid place-items-center rounded-full text-[9.5px] font-mono shrink-0"
                    style={{
                      background: done ? "rgba(74,222,128,0.12)" : active ? "rgba(75,140,255,0.15)" : "rgba(255,255,255,0.04)",
                      color,
                      border: `1px solid ${color}40`,
                    }}
                  >
                    {done ? <Check className="w-2.5 h-2.5" /> : s.n}
                  </span>
                  <span className={`text-[11.5px] truncate ${active ? "text-white/90" : done ? "text-white/55" : "text-white/45"}`}>
                    {s.label}
                  </span>
                </div>
              );
            })}
          </div>
        </section>

        {/* Human Feedback */}
        <section className="px-3 pt-3">
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Human Feedback</div>
          <div className="space-y-1.5">
            {FEEDBACK.map((m, i) => (
              <div
                key={i}
                className="rounded-md border px-2.5 py-1.5"
                style={{
                  background: m.ai ? "rgba(75,140,255,0.05)" : "#15151F",
                  borderColor: m.ai ? "rgba(75,140,255,0.18)" : "rgba(255,255,255,0.06)",
                }}
              >
                <div className="flex items-center gap-1.5 mb-0.5">
                  <span
                    className="text-[10px] font-medium"
                    style={{ color: m.ai ? "#A8C3FF" : "#C8B5FF" }}
                  >
                    {m.who}
                  </span>
                  <span className="text-[10px] text-white/30 font-mono">{m.time}</span>
                </div>
                <div className="text-[11.5px] text-white/75 leading-snug">{m.body}</div>
              </div>
            ))}
          </div>
        </section>

        {/* Pending Approvals */}
        <section className="px-3 pt-3 pb-3">
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">Pending Approvals</span>
            <span
              className="text-[10px] px-1.5 py-[1px] rounded"
              style={{ color: "#FFB86B", background: "rgba(255,184,107,0.08)", border: "1px solid rgba(255,184,107,0.25)" }}
            >
              2 waiting
            </span>
          </div>
          <div className="space-y-1.5">
            {APPROVALS_PAIR.map((a, i) => (
              <div
                key={i}
                className="rounded-lg border p-2.5"
                style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.07)" }}
              >
                <div className="text-[11.5px] text-white/90 leading-snug">{a.title}</div>
                <div className="mt-1 flex items-center gap-1.5 text-[10.5px] text-white/50">
                  <span>{a.agent}</span>
                  <span className="text-white/25">·</span>
                  <span style={{ color: a.riskColor }}>{a.risk} risk</span>
                </div>
                <div className="mt-2 flex items-center gap-1">
                  <button
                    className="h-6 px-2 rounded-md text-[10.5px] font-medium"
                    style={{ background: "rgba(74,222,128,0.12)", color: "#86EFAC", border: "1px solid rgba(74,222,128,0.25)" }}
                  >
                    Approve
                  </button>
                  <button className="h-6 px-2 rounded-md text-[10.5px] text-white/65 bg-white/[0.04] border border-white/[0.08]">
                    Review
                  </button>
                  <button className="h-6 px-2 rounded-md text-[10.5px] text-rose-300 hover:bg-rose-400/10 border border-transparent">
                    Reject
                  </button>
                </div>
              </div>
            ))}
          </div>
        </section>
      </div>

      {/* Composer */}
      <div className="border-t p-3" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div
          className="rounded-lg border focus-within:border-white/20"
          style={{ background: "#101018", borderColor: "rgba(255,255,255,0.08)" }}
        >
          <textarea
            className="w-full bg-transparent resize-none outline-none px-2.5 py-2 text-[11.5px] text-white/85 placeholder:text-white/30"
            rows={2}
            placeholder="Guide the co-pilot, revise the plan, or ask for alternatives…"
          />
          <div
            className="flex items-center justify-between px-2 py-1.5 border-t"
            style={{ borderColor: "rgba(255,255,255,0.05)" }}
          >
            <div className="flex items-center gap-1 text-white/40 text-[10.5px]">
              <button className="h-5 px-1.5 flex items-center gap-1 rounded hover:bg-white/[0.05]">
                <AtSign className="w-3 h-3" /> Context
              </button>
              <span className="text-white/25">·</span>
              <span>pair · GPT-5.5</span>
            </div>
            <button
              className="h-6 px-2 flex items-center gap-1.5 rounded-md text-[10.5px] font-semibold"
              style={{ color: "#09090D", background: "linear-gradient(135deg, #39D7FF 0%, #8B5CFF 100%)" }}
            >
              Send <CornerDownLeft className="w-3 h-3" />
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}

export function RightInspector({ level }: { level: number }) {
  if (level === 1) return <ManualContextInspector level={level} />;
  if (level === 2) return <DelegationConsole level={level} />;
  if (level === 3) return <FleetConsole level={level} />;
  return (
    <div
      className="w-[380px] shrink-0 h-full flex flex-col border-l"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      {/* Header */}
      <div
        className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-2">
          <Sparkles className="w-3.5 h-3.5" style={{ color: "#8B5CFF" }} />
          <span className="text-[12px] text-white/90 font-medium tracking-tight">Directive Console</span>
        </div>
        <span
          className="px-1.5 py-[2px] rounded text-[10px] font-mono"
          style={{ color: "#C8B5FF", background: "rgba(139,92,255,0.10)", border: "1px solid rgba(139,92,255,0.25)" }}
        >
          L{level}
        </span>
      </div>

      {/* Directive composer (top, command palette feel) */}
      <div className="p-3">
        <div
          className="rounded-xl border focus-within:border-white/20 transition"
          style={{ background: "#101018", borderColor: "rgba(255,255,255,0.08)" }}
        >
          <div className="px-3 pt-2.5 flex items-center justify-between">
            <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">Singular Directive</span>
            <span className="text-[10px] text-white/40 font-mono">⌘↵</span>
          </div>
          <textarea
            className="w-full bg-transparent resize-none outline-none px-3 py-2 text-[12.5px] text-white/90 placeholder:text-white/30"
            rows={3}
            placeholder="Describe what you want built, changed, tested, or reviewed…"
            defaultValue="Implement Stripe subscriptions and wire them into the existing user model."
          />
          <div
            className="flex items-center justify-between px-2 py-2 border-t"
            style={{ borderColor: "rgba(255,255,255,0.05)" }}
          >
            <div className="flex items-center gap-1 text-white/45">
              <button className="h-6 px-1.5 flex items-center gap-1 rounded-md hover:bg-white/[0.05] text-[10.5px]">
                <AtSign className="w-3 h-3" /> Context
              </button>
              <button className="h-6 px-1.5 flex items-center gap-1 rounded-md hover:bg-white/[0.05] text-[10.5px]">
                <Paperclip className="w-3 h-3" /> Attach
              </button>
              <button className="h-6 px-1.5 flex items-center gap-1 rounded-md hover:bg-white/[0.05] text-[10.5px]">
                <Shield className="w-3 h-3" /> Constraint
              </button>
            </div>
            <button
              className="h-6 px-2 flex items-center gap-1.5 rounded-md text-[10.5px] font-semibold"
              style={{
                color: "#09090D",
                background: "linear-gradient(135deg, #39D7FF 0%, #8B5CFF 100%)",
              }}
            >
              Run <CornerDownLeft className="w-3 h-3" />
            </button>
          </div>
        </div>

        {/* Context scope */}
        <div className="mt-2 flex items-center gap-1.5 text-[10.5px]">
          <button
            className="h-6 px-2 flex items-center gap-1.5 rounded-md border"
            style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.08)", color: "#B6B7C3" }}
          >
            <Scan className="w-3 h-3" /> Scope: current module
            <ChevronDown className="w-3 h-3 text-white/40" />
          </button>
          <span className="text-white/35">·</span>
          <span className="text-white/45">4 files · auth, billing, models</span>
        </div>
      </div>

      {/* Approval queue */}
      <div className="px-3">
        <div className="flex items-center justify-between mb-1.5">
          <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/35">Approval Queue</span>
          <span
            className="text-[10px] px-1.5 py-[1px] rounded"
            style={{ color: "#FFB86B", background: "rgba(255,184,107,0.08)", border: "1px solid rgba(255,184,107,0.25)" }}
          >
            2 waiting
          </span>
        </div>
        <div className="space-y-1.5">
          {APPROVALS.map((a, i) => (
            <div
              key={i}
              className="rounded-lg border p-2.5"
              style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.07)" }}
            >
              <div className="text-[12px] text-white/90 leading-snug">{a.title}</div>
              <div className="mt-1 flex items-center gap-1.5 text-[10.5px] text-white/50">
                <span>{a.agent}</span>
                <span className="text-white/25">·</span>
                <span style={{ color: a.riskColor }}>{a.risk} risk</span>
                <span className="text-white/25">·</span>
                <span>{a.files} files</span>
              </div>
              <div className="mt-2 flex items-center gap-1">
                <button
                  className="h-6 px-2 rounded-md text-[10.5px] font-medium"
                  style={{ background: "rgba(74,222,128,0.12)", color: "#86EFAC", border: "1px solid rgba(74,222,128,0.25)" }}
                >
                  Approve
                </button>
                <button className="h-6 px-2 rounded-md text-[10.5px] text-white/65 bg-white/[0.04] border border-white/[0.08]">
                  Review
                </button>
                <button className="h-6 px-2 rounded-md text-[10.5px] text-rose-300 hover:bg-rose-400/10 border border-transparent">
                  Reject
                </button>
              </div>
            </div>
          ))}
        </div>
      </div>

      {/* Decision feed + activity */}
      <div className="px-3 mt-3 flex-1 overflow-y-auto space-y-3">
        <div>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Recent Decisions</div>
          <div className="space-y-1.5">
            {DECISIONS.map((d, i) => (
              <div key={i} className="text-[11.5px] text-white/65 leading-snug">
                <span className="text-white/85 font-medium">{d.who}</span>{" "}
                <span className="text-white/50">{d.body}</span>{" "}
                <span className="text-white/30 font-mono text-[10px]">{d.time}</span>
              </div>
            ))}
          </div>
        </div>

        <div>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Agent Activity</div>
          <div
            className="rounded-md border font-mono text-[10.5px] divide-y"
            style={{ background: "#0B0B10", borderColor: "rgba(255,255,255,0.06)" }}
          >
            {ACTIVITY.map((e, i) => (
              <div key={i} className="flex items-start gap-2 px-2 py-1.5" style={{ borderColor: "rgba(255,255,255,0.04)" }}>
                <span className="text-white/30 shrink-0">{e.time}</span>
                <span
                  className="shrink-0 px-1 rounded text-[9.5px] font-semibold tracking-wide"
                  style={{ background: "rgba(255,255,255,0.04)", color: e.color }}
                >
                  {e.tag}
                </span>
                <span className="text-white/70 truncate">{e.body}</span>
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Footer */}
      <div
        className="border-t px-3 py-2 flex items-center justify-between text-[10px] text-white/40"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <span>4 agents engaged · L{level}</span>
        <span className="font-mono">claude-opus-4.7 · 1M ctx</span>
      </div>
    </div>
  );
}

function FleetConsole({ level }: { level: number }) {
  const APPROVALS_FLEET = [
    { title: "Apply schema migration?", files: 1, agent: "Local DB Agent" },
    { title: "Merge generated tests?", files: 3, agent: "Gemini QA Agent" },
    { title: "Accept webhook handler implementation?", files: 1, agent: "Claude Backend" },
  ];
  const DECISIONS_FLEET = [
    { agent: "Backend Team", action: "selected Stripe checkout sessions over payment links", color: "#4B8CFF" },
    { agent: "QA Team", action: "added regression tests for canceled subscriptions", color: "#39D7FF" },
    { agent: "Review Agent", action: "flagged missing idempotency check", color: "#8B5CFF" },
  ];
  return (
    <div
      className="w-[380px] shrink-0 h-full flex flex-col border-l"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      <div
        className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
        style={{ borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-2">
          <Wand2 className="w-3.5 h-3.5" style={{ color: "#B16CFF" }} />
          <span className="text-[12px] text-white/90 font-medium tracking-tight">Legion Workflow Control</span>
        </div>
        <span
          className="px-1.5 py-[2px] rounded text-[10px] font-mono"
          style={{ color: "#D9B8FF", background: "rgba(177,108,255,0.12)", border: "1px solid rgba(177,108,255,0.5)" }}
        >
          L{level}
        </span>
      </div>

      <div className="flex-1 overflow-y-auto px-3 py-3 space-y-4">
        {/* Command palette input */}
        <section>
          <div
            className="rounded-lg border focus-within:border-white/20 transition px-3 py-2 flex items-center gap-2 shadow-sm"
            style={{ background: "#101018", borderColor: "rgba(255,255,255,0.08)" }}
          >
            <Sparkles className="w-4 h-4 text-white/30 shrink-0" />
            <div className="flex-1 min-w-0">
              <input
                className="w-full bg-transparent outline-none text-[12px] text-white/90 placeholder:text-white/30"
                placeholder="Issue a directive, constraint, or correction…"
                defaultValue="Prioritize webhook signature validation before checkout UI"
              />
            </div>
            <span className="text-[10px] text-white/30 font-mono shrink-0 border border-white/10 rounded px-1">↵</span>
          </div>
        </section>

        {/* Current Directive */}
        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-[#B16CFF] mb-1.5 font-medium">Current Directive</div>
          <div className="rounded-lg border p-2.5" style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.06)" }}>
            <div className="text-[12px] text-white/90 leading-snug mb-2">
              Implement Stripe subscriptions and wire them into the existing user model
            </div>
            <div className="flex items-center gap-3 text-[10.5px] text-white/50">
              <span className="flex items-center gap-1.5">
                <div className="w-1.5 h-1.5 rounded-full bg-[#4ADE80] animate-pulse" /> Running
              </span>
              <span className="text-white/20">|</span>
              <span>Started 12m ago</span>
              <span className="text-white/20">|</span>
              <span>87% confidence</span>
            </div>
          </div>
        </section>

        {/* Human Approval Queue */}
        <section>
          <div className="flex items-center justify-between mb-1.5">
            <span className="text-[9.5px] uppercase tracking-[0.18em] text-[#FFB86B]">Human Approval Queue</span>
            <span className="text-[10px] px-1.5 rounded" style={{ background: "rgba(255,184,107,0.1)", color: "#FFB86B" }}>3</span>
          </div>
          <div className="space-y-1.5">
            {APPROVALS_FLEET.map((a, i) => (
              <div key={i} className="rounded-md border p-2.5 flex items-center justify-between group" style={{ background: "#15151F", borderColor: "rgba(255,184,107,0.2)" }}>
                <div className="flex-1 min-w-0 pr-2">
                  <div className="text-[11.5px] text-white/90 leading-snug truncate">{a.title}</div>
                  <div className="text-[10px] text-white/45 mt-0.5">{a.agent} · {a.files} files</div>
                </div>
                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                  <button className="h-6 px-2 rounded bg-[#4ADE80]/10 text-[#86EFAC] text-[10px] border border-[#4ADE80]/20 hover:bg-[#4ADE80]/20">✓</button>
                  <button className="h-6 px-2 rounded bg-white/5 text-white/60 text-[10px] border border-white/10 hover:bg-white/10">✗</button>
                </div>
              </div>
            ))}
          </div>
        </section>

        {/* Agent Decision Feed */}
        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-white/35 mb-1.5">Agent Decision Feed</div>
          <div className="space-y-2 relative before:absolute before:inset-y-1 before:left-[3px] before:w-[1px] before:bg-white/5 pl-4">
            {DECISIONS_FLEET.map((d, i) => (
              <div key={i} className="text-[11px] leading-snug relative">
                <div className="absolute top-1.5 -left-4 w-1.5 h-1.5 rounded-full" style={{ background: d.color }} />
                <span className="font-medium text-white/85">{d.agent}</span> <span className="text-white/55">{d.action}</span>
              </div>
            ))}
          </div>
        </section>

        {/* Risk Monitor */}
        <section>
          <div className="text-[9.5px] uppercase tracking-[0.18em] text-[#FF5C7A] mb-1.5">Risk Monitor</div>
          <div className="rounded-lg border p-2.5 space-y-2" style={{ background: "#15151F", borderColor: "rgba(255,92,122,0.15)" }}>
            <div className="flex items-center justify-between text-[11px]">
              <span className="flex items-center gap-1.5 text-white/70"><Check className="w-3 h-3 text-[#4ADE80]" /> Build Status</span>
              <span className="text-[#4ADE80]">Passing</span>
            </div>
            <div className="flex items-center justify-between text-[11px]">
              <span className="flex items-center gap-1.5 text-white/70"><X className="w-3 h-3 text-[#FF5C7A]" /> Failing Tests</span>
              <span className="text-[#FF5C7A]">2 related to idempotency</span>
            </div>
            <div className="flex items-center justify-between text-[11px]">
              <span className="flex items-center gap-1.5 text-white/70"><Shield className="w-3 h-3 text-[#FFCC66]" /> Security Flags</span>
              <span className="text-[#FFCC66]">1 review required</span>
            </div>
            <div className="flex items-center justify-between text-[11px] pt-1 mt-1 border-t" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
              <span className="text-white/50">High-risk files touched</span>
              <span className="font-mono text-white/70">api/billing/webhook.ts</span>
            </div>
          </div>
        </section>
      </div>
    </div>
  );
}
