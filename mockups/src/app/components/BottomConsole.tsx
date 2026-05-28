import {
  TerminalSquare,
  ScrollText,
  FlaskConical,
  Radio,
  ChevronDown,
  Circle,
  Sparkles,
  Brain,
  Bug,
  ListChecks,
  Wrench,
  Database,
  Globe2,
  ShieldCheck,
  Activity,
  Gauge,
} from "lucide-react";
import { useState } from "react";
import { MANUAL_COMMAND_TARGETS, MANUAL_TOOLCHAIN } from "../manualModeProjection";

const TABS_FULL = [
  { key: "terminal", icon: TerminalSquare, label: "Terminal" },
  { key: "tests", icon: FlaskConical, label: "Tests", badge: "412" },
  { key: "comm", icon: Radio, label: "Agent Comm Stream", live: true },
  { key: "workflow", icon: ScrollText, label: "Workflow Logs" },
];

const TABS_MANUAL = [
  { key: "terminal", icon: TerminalSquare, label: "Terminal" },
  { key: "problems", icon: Bug, label: "Problems", badge: "2" },
  { key: "tasks", icon: ListChecks, label: "Tasks", badge: "8" },
  { key: "tests", icon: FlaskConical, label: "Tests", badge: "412" },
  { key: "debug", icon: Wrench, label: "Debug" },
  { key: "logs", icon: ScrollText, label: "Logs" },
  { key: "observability", icon: Activity, label: "Observability" },
  { key: "database", icon: Database, label: "Database" },
  { key: "api", icon: Globe2, label: "API" },
  { key: "performance", icon: Gauge, label: "Performance" },
  { key: "security", icon: ShieldCheck, label: "Security" },
];

const TABS_DELEGATED = [
  { key: "terminal", icon: TerminalSquare, label: "Terminal" },
  { key: "tests", icon: FlaskConical, label: "Test Runner", badge: "14" },
  { key: "agentlogs", icon: Radio, label: "Agent Logs", live: true },
];

const AGENT_LOGS = [
  { t: "12:08:51", agent: "Backend", color: "#4B8CFF", body: "Ran migration validation on user_billing — schema diff looks safe." },
  { t: "12:08:33", agent: "QA", color: "#39D7FF", body: "Generated 8 test cases for subscription lifecycle (create / update / cancel / pause)." },
  { t: "12:08:11", agent: "Review", color: "#8B5CFF", body: "Flagged potential race condition in webhook handler — recommends idempotency guard." },
  { t: "12:07:42", agent: "Frontend", color: "#39D7FF", body: "Implemented BillingSettings.tsx — wired to /api/billing/subscription." },
  { t: "12:07:18", agent: "Backend", color: "#4B8CFF", body: "Updated subscription controller — added eventVersion field for idempotency." },
  { t: "12:06:54", agent: "Review", color: "#8B5CFF", body: "Inspected diff a4f1c2e — 1 nit, 0 blockers." },
  { t: "12:06:21", agent: "QA", color: "#39D7FF", body: "Test run · 12/14 passing · 2 failures pinned to webhook handler." },
];

const TABS_COPILOT = [
  { key: "terminal", icon: TerminalSquare, label: "Terminal" },
  { key: "tests", icon: FlaskConical, label: "Tests", badge: "412" },
  { key: "reasoning", icon: Brain, label: "Reasoning Summary", live: true },
  { key: "workflow", icon: ScrollText, label: "Output" },
];

const TABS_ASSISTED = [
  { key: "terminal", icon: TerminalSquare, label: "Terminal" },
  { key: "tests", icon: FlaskConical, label: "Tests", badge: "412" },
  { key: "ai", icon: Sparkles, label: "AI Suggestions Log", badge: "12" },
  { key: "workflow", icon: ScrollText, label: "Output" },
];

const AI_SUGGESTIONS = [
  { t: "12:08:21", kind: "fix", color: "#FFCC66", file: "subscriptions.ts", body: "Suggested null guard in createSubscription", accepted: true },
  { t: "12:07:54", kind: "doc", color: "#4B8CFF", file: "auth.ts", body: "Generated docstring for validateToken", accepted: true },
  { t: "12:07:11", kind: "warn", color: "#FF5C7A", file: "middleware/auth.ts", body: "Detected unhandled promise in auth middleware", accepted: false },
  { t: "12:06:42", kind: "test", color: "#39D7FF", file: "billing.test.ts", body: "Generated vitest for cancelSubscription path", accepted: true },
  { t: "12:06:08", kind: "ref", color: "#8B5CFF", file: "subscriptions.ts", body: "Suggested extracting validateCustomer() helper", accepted: false },
  { t: "12:05:30", kind: "fix", color: "#FFCC66", file: "user.ts", body: "Narrow role to literal union 'admin' | 'editor' | 'viewer'", accepted: true },
];

const TERMINAL_MANUAL = [
  { p: "~/nebula-commerce", c: "pnpm run dev" },
  { o: "> nebula-commerce@0.4.2 dev", color: "#7E8190" },
  { o: "> next dev --turbo", color: "#7E8190" },
  { o: "  ▲ Next.js 15.0.3 (turbo)", color: "#B6B7C3" },
  { o: "  - Local:    http://localhost:3000", color: "#39D7FF" },
  { o: "  - Network:  http://192.168.1.24:3000", color: "#7E8190" },
  { o: " ✓ Ready in 412ms", color: "#4ADE80" },
  { o: " ✓ Compiled /api/auth in 184ms (612 modules)", color: "#4ADE80" },
  { o: " ○ tests passing · 412 / 412", color: "#4ADE80" },
];

const TERMINAL = [
  { p: "~/nebula-commerce", c: "pnpm test --filter billing" },
  { o: "✓ billing/checkout.test.ts (14)", color: "#4ADE80" },
  { o: "✓ billing/webhook.test.ts (8)", color: "#4ADE80" },
  { o: "Test Files  2 passed (2)", color: "#B6B7C3" },
  { o: "Tests       22 passed (22)", color: "#B6B7C3" },
  { o: "Duration    1.84s", color: "#7E8190" },
];

const TERMINAL_COPILOT = [
  { p: "~/nebula-commerce", c: "npm test auth.test.ts" },
  { o: "> nebula-commerce@0.4.2 test", color: "#7E8190" },
  { o: "> vitest run auth.test.ts", color: "#7E8190" },
  { o: " RUN  v1.6.0  /workspaces/nebula-commerce", color: "#B6B7C3" },
  { o: " ✓ auth/middleware.test.ts (22)", color: "#4ADE80" },
  { o: "   ✓ rejects expired token", color: "#4ADE80" },
  { o: "   ✓ rejects missing token", color: "#4ADE80" },
  { o: "   ✓ accepts valid token within leeway", color: "#4ADE80" },
  { o: " Test Files  1 passed (1)", color: "#B6B7C3" },
  { o: " Tests       22 passed (22)", color: "#B6B7C3" },
  { o: " Duration    0.94s", color: "#7E8190" },
];

const REASONING = [
  { t: "12:04:11", who: "GPT-5.5", color: "#4B8CFF", body: "Detected existing JWT helper in lib/jwt.ts — reusing verify() rather than re-rolling." },
  { t: "12:04:38", who: "GPT-5.5", color: "#4B8CFF", body: "Selected middleware insertion point after token decode, before role check." },
  { t: "12:05:02", who: "Claude", color: "#8B5CFF", body: "Reviewed approach — recommends 30s leeway on exp claim to absorb clock skew." },
  { t: "12:05:44", who: "GPT-5.5", color: "#4B8CFF", body: "Added isExpired(exp, leeway) helper inline; matches your earlier guidance to keep it colocated." },
  { t: "12:06:21", who: "Local", color: "#39D7FF", body: "Indexed call sites of validateToken — 4 callers, none rely on prior behavior." },
  { t: "12:07:08", who: "GPT-5.5", color: "#4B8CFF", body: "Generated tests for expired token and missing token paths; running suite next." },
];

const COMM = [
  { t: "12:04:11", tag: "PLAN", color: "#FFCC66", body: "Planner → Backend Team: Assigned checkout session endpoint" },
  { t: "12:04:18", tag: "WRITE", color: "#4B8CFF", body: "Backend Agent: Added Stripe SDK integration" },
  { t: "12:04:31", tag: "TEST", color: "#39D7FF", body: "QA Agent: Generated 6 subscription lifecycle tests" },
  { t: "12:04:44", tag: "REVIEW", color: "#8B5CFF", body: "Review Agent: Flagged missing webhook idempotency guard" },
  { t: "12:05:02", tag: "APPROVAL", color: "#FFB86B", body: "Human approval requested · apply migration" },
  { t: "12:05:19", tag: "COMPLETE", color: "#4ADE80", body: "Test Runner: billing.test.ts passed 14/14" },
];

export function BottomConsole({ level = 3 }: { level?: number }) {
  const manual = level === 1;
  const assisted = false;
  const copilot = false;
  const delegated = level === 2;
  const fleet = level === 3;
  if (fleet) return <FleetBottomConsole level={level} />;
  const TABS = manual ? TABS_MANUAL : assisted ? TABS_ASSISTED : copilot ? TABS_COPILOT : delegated ? TABS_DELEGATED : TABS_FULL;
  const [tab, setTab] = useState(manual ? "terminal" : assisted ? "ai" : copilot ? "reasoning" : delegated ? "agentlogs" : "comm");
  const effectiveTab = TABS.find((t) => t.key === tab) ? tab : TABS[0].key;
  const manualDiagnostics = MANUAL_TOOLCHAIN.flatMap((tool) => tool.diagnostics);
  return (
    <div
      className="shrink-0 border-t flex flex-col"
      style={{ height: manual ? 260 : 200, background: "#0B0B10", borderColor: "rgba(255,255,255,0.05)" }}
    >
      <div className="h-8 shrink-0 flex items-center justify-between px-2 border-b" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div className="flex items-center gap-0.5">
          {TABS.map((t) => {
            const active = tab === t.key;
            return (
              <button
                key={t.key}
                onClick={() => setTab(t.key)}
                className={`h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] ${
                  effectiveTab === t.key ? "text-white/90" : "text-white/45 hover:text-white/75"
                }`}
                style={{ background: effectiveTab === t.key ? "rgba(255,255,255,0.05)" : "transparent" }}
              >
                <t.icon className="w-3.5 h-3.5" />
                {t.label}
                {t.badge && (
                  <span className="ml-1 px-1 rounded text-[9.5px] text-white/55 bg-white/[0.05]">{t.badge}</span>
                )}
                {t.live && (
                  <span className="ml-1 flex items-center gap-1 text-[9.5px]" style={{ color: "#4ADE80" }}>
                    <Circle className="w-1.5 h-1.5 fill-current" /> live
                  </span>
                )}
              </button>
            );
          })}
        </div>
        <div className="flex items-center gap-2 text-[10.5px] text-white/40 pr-1">
          <span className="font-mono">{manual ? "local · no model calls" : "us-west · 42 ms"}</span>
          <button className="h-6 w-6 grid place-items-center rounded-md hover:bg-white/[0.05]">
            <ChevronDown className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      <div className="flex-1 overflow-y-auto font-mono text-[11.5px] leading-[1.7] py-1.5 px-3">
        {effectiveTab === "terminal" &&
          (manual ? TERMINAL_MANUAL : copilot ? TERMINAL_COPILOT : TERMINAL).map((l, i) =>
            l.c ? (
              <div key={i} className="flex gap-2">
                <span style={{ color: "#8B5CFF" }}>{l.p}</span>
                <span className="text-white/35">$</span>
                <span className="text-white/90">{l.c}</span>
              </div>
            ) : (
              <div key={i} style={{ color: l.color }}>{l.o}</div>
            )
          )}
        {effectiveTab === "terminal" && manual && (
          <div className="flex gap-2 mt-1">
            <span style={{ color: "#8B5CFF" }}>~/nebula-commerce</span>
            <span className="text-white/35">$</span>
            <span className="w-1.5 h-3 inline-block animate-pulse" style={{ background: "#B6B7C3" }} />
          </div>
        )}

        {effectiveTab === "tests" && (
          <div className="space-y-0.5 text-white/75">
            <div style={{ color: "#4ADE80" }}>✓ billing/checkout.test.ts <span className="text-white/40">(14)</span></div>
            <div style={{ color: "#4ADE80" }}>✓ billing/webhook.test.ts <span className="text-white/40">(8)</span></div>
            <div style={{ color: "#4ADE80" }}>✓ auth/session.test.ts <span className="text-white/40">(19)</span></div>
            <div style={{ color: "#4ADE80" }}>✓ models/user.test.ts <span className="text-white/40">(11)</span></div>
            <div className="text-white/45 mt-1">412 passed · 0 failed · coverage 91.4%</div>
          </div>
        )}

        {effectiveTab === "problems" && manual && (
          <div className="space-y-1">
            {manualDiagnostics.map((problem) => (
              <div key={`${problem.source}-${problem.target}`} className="grid grid-cols-[72px_180px_1fr] gap-3 items-start">
                <span className="uppercase tracking-wide text-[10px]" style={{ color: problem.severity === "warning" ? "#FFCC66" : "#4B8CFF" }}>
                  {problem.severity}
                </span>
                <span className="font-mono text-white/45 truncate">{problem.target}</span>
                <span className="text-white/75 truncate">{problem.message}</span>
              </div>
            ))}
            <div className="text-white/40 mt-1">LSP diagnostics, linter output, task matchers, test failures, and stack traces merge here.</div>
          </div>
        )}

        {effectiveTab === "tasks" && manual && (
          <div className="grid grid-cols-2 gap-x-8 gap-y-1 text-white/75">
            {[
              ["dev", "pnpm dev", "running"],
              ["test", "pnpm test", "ready"],
              ["typecheck", "tsc --noEmit", "ready"],
              ["lint", "eslint .", "ready"],
              ["build", "next build", "ready"],
              ["db:migrate", "prisma migrate dev", "blocked until confirmed"],
            ].map(([name, command, state]) => (
              <div key={name} className="grid grid-cols-[96px_1fr_130px] gap-3">
                <span className="font-mono text-white/85">{name}</span>
                <span className="font-mono text-white/45 truncate">{command}</span>
                <span style={{ color: state === "running" ? "#4ADE80" : state === "ready" ? "#4B8CFF" : "#FFCC66" }}>{state}</span>
              </div>
            ))}
            <div className="col-span-2 text-white/40 mt-1">Problem matchers map compiler and task output back to source ranges.</div>
          </div>
        )}

        {effectiveTab === "debug" && manual && (
          <div className="grid grid-cols-[1fr_1fr_1fr] gap-5 text-white/75">
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Call Stack</div>
              <div>node:9229 · paused at <span className="font-mono text-white/60">auth.ts:23</span></div>
              <div className="text-white/45">issueSession → jwtVerify → readSession</div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Breakpoints</div>
              <div>2 enabled · 1 conditional · 1 logpoint</div>
              <div className="text-white/45">auth.ts:23 · webhook.ts:41</div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Watch</div>
              <div><span className="font-mono">payload.sub</span> = "usr_42"</div>
              <div><span className="font-mono">payload.role</span> = "admin"</div>
            </div>
          </div>
        )}

        {effectiveTab === "logs" && manual && (
          <div className="space-y-0.5">
            {[
              ["12:08:51", "web", "GET /api/auth/session 200 18ms"],
              ["12:08:53", "api", "POST /billing/checkout 201 96ms"],
              ["12:08:54", "worker", "subscription.sync completed event=sub_created"],
              ["12:08:56", "postgres", "slow query 142ms public.subscriptions"],
            ].map(([time, source, body]) => (
              <div key={`${time}-${source}`} className="flex gap-3">
                <span className="text-white/30">{time}</span>
                <span className="w-16 text-[#4B8CFF]">{source}</span>
                <span className="text-white/75 truncate">{body}</span>
              </div>
            ))}
          </div>
        )}

        {effectiveTab === "observability" && manual && (
          <div className="grid grid-cols-[1.2fr_1fr] gap-6 text-white/75">
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Request Timeline</div>
              <div>POST /billing/checkout · 96ms · trace <span className="font-mono text-white/55">5f43a</span></div>
              <div className="text-white/45">controller 12ms → db 31ms → stripe 42ms → serialize 4ms</div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">OpenTelemetry</div>
              <div>traces idle · metrics live · logs live</div>
              <div className="text-white/45">local collector · no hosted export</div>
            </div>
          </div>
        )}

        {effectiveTab === "database" && manual && (
          <div className="grid grid-cols-[220px_1fr] gap-5 text-white/75">
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Schema</div>
              <div className="font-mono">public.users</div>
              <div className="font-mono">public.subscriptions</div>
              <div className="font-mono">public.webhook_events</div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Query</div>
              <div className="font-mono text-white/60">select status, count(*) from subscriptions group by status;</div>
              <div className="text-white/45">Explain plan ready · safe local connection</div>
            </div>
          </div>
        )}

        {effectiveTab === "api" && manual && (
          <div className="grid grid-cols-[1fr_1fr] gap-6 text-white/75">
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">billing.http</div>
              <div className="font-mono">POST {"{{apiUrl}}"}/billing/checkout</div>
              <div className="text-white/45">env: local · auth profile: dev-user</div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Response</div>
              <div>201 Created · 96ms · 1.4 KB</div>
              <div className="text-white/45">headers, cookies, timeline, assertions available</div>
            </div>
          </div>
        )}

        {effectiveTab === "performance" && manual && (
          <div className="grid grid-cols-4 gap-5 text-white/75">
            {[
              ["CPU", "18%", "profiler ready"],
              ["Memory", "412 MB", "heap snapshot ready"],
              ["Bundle", "184 KB", "analyzer ready"],
              ["Queries", "1 slow", "142 ms max"],
            ].map(([label, value, note]) => (
              <div key={label}>
                <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">{label}</div>
                <div className="font-mono text-white/90">{value}</div>
                <div className="text-white/45">{note}</div>
              </div>
            ))}
          </div>
        )}

        {effectiveTab === "security" && manual && (
          <div className="grid grid-cols-[1fr_1fr] gap-6 text-white/75">
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Supply Chain</div>
              <div>0 critical vulnerabilities · 0 secrets · SBOM available</div>
              <div className="text-white/45">license review clean · lockfile diff clean</div>
            </div>
            <div>
              <div className="text-[10px] uppercase tracking-[0.16em] text-white/35 mb-1">Policy Gates</div>
              <div>No lifecycle script alerts</div>
              <div className="text-white/45">Manual Mode blocks AI-capable extension activation</div>
            </div>
          </div>
        )}

        {effectiveTab === "comm" &&
          COMM.map((l, i) => (
            <div key={i} className="flex gap-3">
              <span className="text-white/30">{l.t}</span>
              <span
                className="shrink-0 px-1 rounded text-[10px] font-semibold tracking-wide"
                style={{ background: "rgba(255,255,255,0.04)", color: l.color }}
              >
                {l.tag}
              </span>
              <span className="text-white/70 truncate">{l.body}</span>
            </div>
          ))}

        {effectiveTab === "ai" && (
          <div className="space-y-0.5">
            {AI_SUGGESTIONS.map((s, i) => (
              <div key={i} className="flex items-center gap-3">
                <span className="text-white/30">{s.t}</span>
                <span
                  className="shrink-0 px-1 rounded text-[10px] font-semibold tracking-wide uppercase"
                  style={{ background: "rgba(255,255,255,0.04)", color: s.color }}
                >
                  {s.kind}
                </span>
                <span className="text-white/55 w-[160px] truncate font-mono text-[10.5px]">{s.file}</span>
                <span className="text-white/75 truncate flex-1">{s.body}</span>
                <span className="shrink-0 text-[10px]" style={{ color: s.accepted ? "#4ADE80" : "#7E8190" }}>
                  {s.accepted ? "accepted" : "dismissed"}
                </span>
              </div>
            ))}
            <div className="flex gap-3 items-center mt-1 text-white/45">
              <span className="text-white/30">12:08:42</span>
              <span className="px-1 rounded text-[10px] font-semibold uppercase" style={{ background: "rgba(255,255,255,0.04)", color: "#9EE9FF" }}>
                idle
              </span>
              <span>watching subscriptions.ts for changes</span>
            </div>
          </div>
        )}
        {effectiveTab === "agentlogs" && (
          <div className="space-y-0.5">
            {AGENT_LOGS.map((l, i) => (
              <div key={i} className="flex items-start gap-3">
                <span className="text-white/30 shrink-0">{l.t}</span>
                <span
                  className="shrink-0 px-1 rounded text-[10px] font-semibold tracking-wide"
                  style={{ background: `${l.color}1F`, color: l.color, border: `1px solid ${l.color}55` }}
                >
                  {l.agent}
                </span>
                <span className="text-white/75 leading-snug">{l.body}</span>
              </div>
            ))}
            <div className="flex gap-3 items-center mt-1 text-white/45">
              <span className="text-white/30">12:08:58</span>
              <span className="px-1 rounded text-[10px] font-semibold" style={{ background: "rgba(177,108,255,0.10)", color: "#D9B8FF", border: "1px solid rgba(177,108,255,0.30)" }}>
                FLEET
              </span>
              <span>4 agents working · 3 approvals pending</span>
              <span className="w-1.5 h-3 animate-pulse" style={{ background: "#8B5CFF" }} />
            </div>
          </div>
        )}

        {effectiveTab === "reasoning" && (
          <div className="space-y-1">
            <div className="flex items-center gap-2 mb-1 text-[10px] text-white/40">
              <Brain className="w-3 h-3" style={{ color: "#4B8CFF" }} />
              <span>Summaries · not raw chain-of-thought</span>
              <span className="text-white/25">·</span>
              <span className="font-mono">npm test auth.test.ts</span>
            </div>
            {REASONING.map((r, i) => (
              <div key={i} className="flex items-start gap-3">
                <span className="text-white/30 shrink-0">{r.t}</span>
                <span
                  className="shrink-0 px-1 rounded text-[10px] font-semibold tracking-wide"
                  style={{ background: "rgba(255,255,255,0.04)", color: r.color }}
                >
                  {r.who}
                </span>
                <span className="text-white/75 leading-snug">{r.body}</span>
              </div>
            ))}
            <div className="flex gap-3 items-center mt-1 text-white/45">
              <span className="text-white/30">12:07:42</span>
              <span className="px-1 rounded text-[10px] font-semibold" style={{ background: "rgba(255,255,255,0.04)", color: "#A8C3FF" }}>
                THINKING
              </span>
              <span>composing patch for tests/auth.test.ts</span>
              <span className="w-1.5 h-3 animate-pulse" style={{ background: "#4B8CFF" }} />
            </div>
          </div>
        )}

        {effectiveTab === "workflow" && (
          <div className="space-y-0.5 text-white/65">
            <div><span className="text-white/35">[wf:stripe-subs]</span> step 3/5 · execute · GPT-5.5 · 00:01:24</div>
            <div><span className="text-white/35">[wf:stripe-subs]</span> step 2/5 · plan · Claude · ok</div>
            <div><span className="text-white/35">[wf:audit-security]</span> step 1/4 · scan · Gemini · queued</div>
          </div>
        )}

        {effectiveTab === "comm" && (
          <div className="flex gap-3 items-center mt-1">
            <span className="text-white/30">12:05:21</span>
            <span className="shrink-0 px-1 rounded text-[10px] font-semibold tracking-wide" style={{ background: "rgba(255,255,255,0.04)", color: "#B6B7C3" }}>
              IDLE
            </span>
            <span className="text-white/55">Legion workflow standing by</span>
            <span className="w-1.5 h-3 animate-pulse" style={{ background: "#39D7FF" }} />
          </div>
        )}
      </div>
    </div>
  );
}

const FLEET_TERMINAL = [
  { p: "~/nebula-commerce", c: "npm install stripe" },
  { o: "added 1 package, and audited 184 packages in 2s", color: "#4ADE80" },
  { p: "~/nebula-commerce", c: "npm run test:billing" },
  { o: "> nebula-commerce@0.4.2 test:billing", color: "#7E8190" },
  { o: "> vitest run --filter billing", color: "#7E8190" },
  { o: "✓ billing/checkout.test.ts (14)", color: "#4ADE80" },
  { o: "✓ billing/webhook.test.ts (8)", color: "#4ADE80" },
  { o: "Test Files  2 passed (2)", color: "#B6B7C3" },
  { o: "Tests       22 passed (22)", color: "#B6B7C3" },
  { o: "Duration    1.84s", color: "#7E8190" },
  { p: "~/nebula-commerce", c: "npm run lint" },
  { o: "> next lint", color: "#7E8190" },
  { o: "✓ No ESLint warnings or errors", color: "#4ADE80" },
];

const FLEET_COMM = [
  { t: "12:04:11", who: "Planner", color: "#8B5CFF", action: "assigned webhook validation to", target: "Backend Agent" },
  { t: "12:04:31", who: "QA Agent", color: "#39D7FF", action: "generated edge case tests", target: "" },
  { t: "12:04:44", who: "Review Agent", color: "#8B5CFF", action: "requested idempotency guard", target: "" },
  { t: "12:06:18", who: "Backend Agent", color: "#4B8CFF", action: "patched failing test", target: "" },
];

function FleetBottomConsole({ level }: { level: number }) {
  return (
    <div
      className="shrink-0 border-t flex"
      style={{ height: 240, background: "#0B0B10", borderColor: "rgba(255,255,255,0.05)" }}
    >
      {/* Left: Terminal */}
      <div className="flex-1 flex flex-col min-w-0 border-r" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <div className="h-8 shrink-0 flex items-center justify-between px-3 border-b" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
          <div className="flex items-center gap-1.5 text-[11px] text-white/90">
            <TerminalSquare className="w-3.5 h-3.5" /> Terminal
          </div>
        </div>
        <div className="flex-1 overflow-y-auto font-mono text-[11.5px] leading-[1.7] py-1.5 px-3">
          {FLEET_TERMINAL.map((l, i) =>
            l.c ? (
              <div key={i} className="flex gap-2">
                <span style={{ color: "#B16CFF" }}>{l.p}</span>
                <span className="text-white/35">$</span>
                <span className="text-white/90">{l.c}</span>
              </div>
            ) : (
              <div key={i} style={{ color: l.color }}>{l.o}</div>
            )
          )}
          <div className="flex gap-2 mt-1">
            <span style={{ color: "#B16CFF" }}>~/nebula-commerce</span>
            <span className="text-white/35">$</span>
            <span className="w-1.5 h-3 inline-block animate-pulse" style={{ background: "#B6B7C3" }} />
          </div>
        </div>
      </div>

      {/* Right: Comm Stream */}
      <div className="flex-1 flex flex-col min-w-0">
        <div className="h-8 shrink-0 flex items-center justify-between px-3 border-b" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
          <div className="flex items-center gap-1.5 text-[11px] text-white/90">
            <Radio className="w-3.5 h-3.5 text-[#B16CFF]" /> Agent Comm Stream
          </div>
          <span className="flex items-center gap-1 text-[9.5px]" style={{ color: "#4ADE80" }}>
            <Circle className="w-1.5 h-1.5 fill-current" /> live
          </span>
        </div>
        <div className="flex-1 overflow-y-auto font-mono text-[11.5px] py-2 px-3 space-y-2">
          {FLEET_COMM.map((l, i) => (
            <div key={i} className="flex gap-3">
              <span className="text-white/30 shrink-0">{l.t}</span>
              <span className="text-white/80 leading-snug">
                <span style={{ color: l.color, fontWeight: 500 }}>{l.who}</span>{" "}
                <span className="text-white/55">{l.action}</span>
                {l.target && (
                   <> <span style={{ color: "#FFB86B", fontWeight: 500 }}>{l.target}</span></>
                )}
              </span>
            </div>
          ))}
          <div className="flex gap-3 items-center pt-1 text-white/45">
            <span className="text-white/30">12:06:58</span>
            <span className="text-white/55">Legion workflow actively communicating</span>
            <span className="w-1.5 h-3 animate-pulse" style={{ background: "#B16CFF" }} />
          </div>
        </div>
      </div>
    </div>
  );
}
