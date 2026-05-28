import { X, Circle, ChevronRight, ChevronDown, Sparkles, Check, CornerDownLeft, Wand2, Lightbulb, TestTube2, FileText as FileTextIcon, Plus, Minus } from "lucide-react";

type Line = { n: number; html: string; gutter?: "add" | "mod" | "ai"; commentary?: string; ghost?: string; selected?: boolean };

const TABS_MANUAL = [
  { name: "auth.ts", path: "api/auth", active: true, dirty: true },
  { name: "subscriptions.ts", path: "api/billing" },
  { name: "user.ts", path: "models" },
];

const TABS_AI = [
  { name: "checkout.ts", path: "api/billing", active: true, dirty: true },
  { name: "webhook.ts", path: "api/billing" },
  { name: "user.ts", path: "models" },
];

const CODE_MANUAL: Line[] = [
  { n: 1, html: `<span class="tk-key">import</span> { <span class="tk-id">cookies</span> } <span class="tk-key">from</span> <span class="tk-str">"next/headers"</span>;` },
  { n: 2, html: `<span class="tk-key">import</span> { <span class="tk-id">jwtVerify</span>, <span class="tk-id">SignJWT</span> } <span class="tk-key">from</span> <span class="tk-str">"jose"</span>;` },
  { n: 3, html: `<span class="tk-key">import</span> { <span class="tk-typ">User</span> } <span class="tk-key">from</span> <span class="tk-str">"@/models/user"</span>;` },
  { n: 4, html: `&nbsp;` },
  { n: 5, html: `<span class="tk-key">const</span> SECRET = <span class="tk-key">new</span> <span class="tk-typ">TextEncoder</span>().<span class="tk-fn">encode</span>(process.env.<span class="tk-id">AUTH_SECRET</span>!);` },
  { n: 6, html: `<span class="tk-key">const</span> COOKIE = <span class="tk-str">"nebula_session"</span>;` },
  { n: 7, html: `&nbsp;` },
  { n: 8, html: `<span class="tk-cmt">/** Sign a session token for the given user. */</span>` },
  { n: 9, html: `<span class="tk-key">export async function</span> <span class="tk-fn">issueSession</span>(user: <span class="tk-typ">User</span>): <span class="tk-typ">Promise</span>&lt;<span class="tk-typ">string</span>&gt; {` },
  { n: 10, html: `&nbsp;&nbsp;<span class="tk-key">const</span> token = <span class="tk-key">await new</span> <span class="tk-fn">SignJWT</span>({ sub: user.id, role: user.role })` },
  { n: 11, html: `&nbsp;&nbsp;&nbsp;&nbsp;.<span class="tk-fn">setProtectedHeader</span>({ alg: <span class="tk-str">"HS256"</span> })` },
  { n: 12, html: `&nbsp;&nbsp;&nbsp;&nbsp;.<span class="tk-fn">setIssuedAt</span>()` },
  { n: 13, html: `&nbsp;&nbsp;&nbsp;&nbsp;.<span class="tk-fn">setExpirationTime</span>(<span class="tk-str">"7d"</span>)` },
  { n: 14, html: `&nbsp;&nbsp;&nbsp;&nbsp;.<span class="tk-fn">sign</span>(SECRET);` },
  { n: 15, html: `&nbsp;&nbsp;<span class="tk-fn">cookies</span>().<span class="tk-fn">set</span>(COOKIE, token, { httpOnly: <span class="tk-key">true</span>, secure: <span class="tk-key">true</span>, sameSite: <span class="tk-str">"lax"</span> });` },
  { n: 16, html: `&nbsp;&nbsp;<span class="tk-key">return</span> token;` },
  { n: 17, html: `}` },
  { n: 18, html: `&nbsp;` },
  { n: 19, html: `<span class="tk-key">export async function</span> <span class="tk-fn">readSession</span>(): <span class="tk-typ">Promise</span>&lt;{ sub: <span class="tk-typ">string</span>; role: <span class="tk-typ">string</span> } | <span class="tk-key">null</span>&gt; {` },
  { n: 20, html: `&nbsp;&nbsp;<span class="tk-key">const</span> token = <span class="tk-fn">cookies</span>().<span class="tk-fn">get</span>(COOKIE)?.value;` },
  { n: 21, html: `&nbsp;&nbsp;<span class="tk-key">if</span> (!token) <span class="tk-key">return null</span>;` },
  { n: 22, html: `&nbsp;&nbsp;<span class="tk-key">try</span> {` },
  { n: 23, html: `&nbsp;&nbsp;&nbsp;&nbsp;<span class="tk-key">const</span> { payload } = <span class="tk-key">await</span> <span class="tk-fn">jwtVerify</span>(token, SECRET);`, ghost: "    return payload as { sub: string; role: string };" },
  { n: 24, html: `&nbsp;&nbsp;} <span class="tk-key">catch</span> {` },
  { n: 25, html: `&nbsp;&nbsp;&nbsp;&nbsp;<span class="tk-key">return null</span>;` },
  { n: 26, html: `&nbsp;&nbsp;}` },
  { n: 27, html: `}` },
];

const CODE_AI: Line[] = [
  { n: 1, html: `<span class="tk-key">import</span> { <span class="tk-id">Agent</span>, <span class="tk-id">FleetEvent</span> } <span class="tk-key">from</span> <span class="tk-str">"./agent"</span>;` },
  { n: 2, html: `<span class="tk-key">import</span> { <span class="tk-id">Protocol</span> } <span class="tk-key">from</span> <span class="tk-str">"./protocol"</span>;` },
  { n: 3, html: `&nbsp;` },
  { n: 4, html: `<span class="tk-key">export class</span> <span class="tk-typ">Fleet</span> {` },
  { n: 5, html: `&nbsp;&nbsp;<span class="tk-key">async</span> <span class="tk-fn">dispatch</span>(directive: <span class="tk-typ">string</span>) {`, gutter: "mod" },
  { n: 6, html: `&nbsp;&nbsp;&nbsp;&nbsp;<span class="tk-key">const</span> tasks = <span class="tk-key">await this</span>.<span class="tk-fn">plan</span>(directive);`, gutter: "add" },
  { n: 7, html: `&nbsp;&nbsp;&nbsp;&nbsp;<span class="tk-key">return</span> <span class="tk-typ">Promise</span>.<span class="tk-fn">all</span>(tasks.<span class="tk-fn">map</span>(t =&gt; <span class="tk-key">this</span>.<span class="tk-fn">assign</span>(t).<span class="tk-fn">run</span>()));`, gutter: "add", commentary: "Orion proposed parallel decomposition · 18 lines · 2 files" },
  { n: 8, html: `&nbsp;&nbsp;}` },
  { n: 9, html: `}` },
];

const TABS_ASSISTED = [
  { name: "subscriptions.ts", path: "api/billing", active: true, dirty: true },
  { name: "auth.ts", path: "api/auth" },
  { name: "user.ts", path: "models" },
];

const CODE_ASSISTED: Line[] = [
  { n: 1, html: `<span class="tk-key">import</span> { <span class="tk-id">stripe</span> } <span class="tk-key">from</span> <span class="tk-str">"@/lib/stripe"</span>;` },
  { n: 2, html: `<span class="tk-key">import</span> { <span class="tk-typ">User</span> } <span class="tk-key">from</span> <span class="tk-str">"@/models/user"</span>;` },
  { n: 3, html: `<span class="tk-key">import</span> { <span class="tk-fn">db</span> } <span class="tk-key">from</span> <span class="tk-str">"@/db"</span>;` },
  { n: 4, html: `&nbsp;` },
  { n: 5, html: `<span class="tk-key">export async function</span> <span class="tk-fn">createSubscription</span>(user: <span class="tk-typ">User</span>, priceId: <span class="tk-typ">string</span>) {` },
  { n: 6, html: `&nbsp;&nbsp;<span class="tk-key">const</span> customerId = user.stripeCustomerId;` },
  { n: 7, html: `&nbsp;&nbsp;<span class="tk-key">const</span> subscription = <span class="tk-key">await</span> stripe.subscriptions.<span class="tk-fn">create</span>({`, selected: true },
  { n: 8, html: `&nbsp;&nbsp;&nbsp;&nbsp;customer: customerId,` },
  { n: 9, html: `&nbsp;&nbsp;&nbsp;&nbsp;items: [{ price: priceId }],`, ghost: "" },
  { n: 10, html: `&nbsp;&nbsp;&nbsp;&nbsp;payment_behavior: <span class="tk-str">"default_incomplete"</span>,`, ghost: "    expand: [\"latest_invoice.payment_intent\"]," },
  { n: 11, html: `&nbsp;&nbsp;});` },
  { n: 12, html: `&nbsp;` },
  { n: 13, html: `&nbsp;&nbsp;<span class="tk-key">await</span> db.subscriptions.<span class="tk-fn">insert</span>({`, ghost: "" },
  { n: 14, html: `&nbsp;&nbsp;&nbsp;&nbsp;userId: user.id,`, ghost: "" },
  { n: 15, html: `&nbsp;&nbsp;&nbsp;&nbsp;stripeSubscriptionId: subscription.id,`, ghost: "" },
  { n: 16, html: `&nbsp;&nbsp;&nbsp;&nbsp;status: subscription.status,`, ghost: "    currentPeriodEnd: new Date(subscription.current_period_end * 1000)," },
  { n: 17, html: `&nbsp;&nbsp;});` },
  { n: 18, html: `&nbsp;` },
  { n: 19, html: `&nbsp;&nbsp;<span class="tk-key">return</span> subscription;` },
  { n: 20, html: `}` },
  { n: 21, html: `&nbsp;` },
  { n: 22, html: `<span class="tk-key">export async function</span> <span class="tk-fn">cancelSubscription</span>(subscriptionId: <span class="tk-typ">string</span>) {` },
  { n: 23, html: `&nbsp;&nbsp;<span class="tk-key">return</span> stripe.subscriptions.<span class="tk-fn">cancel</span>(subscriptionId);` },
  { n: 24, html: `}` },
];

export function CodeCanvas({ level = 3 }: { level?: number }) {
  const manual = level === 1;
  const assisted = level === 2;
  const copilot = level === 3;
  if (copilot) return <CopilotCanvas />;
  if (level === 4) return <DelegatedCanvas />;
  if (level === 5) return <FleetCanvas />;
  const TABS = manual ? TABS_MANUAL : assisted ? TABS_ASSISTED : TABS_AI;
  const CODE = manual ? CODE_MANUAL : assisted ? CODE_ASSISTED : CODE_AI;
  const activeTab = TABS.find((t) => t.active)!;

  return (
    <div className="flex-1 min-w-0 h-full flex flex-col" style={{ background: "#0B0B10" }}>
      {/* Tabs */}
      <div
        className="h-9 shrink-0 flex items-end px-2 border-b"
        style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
      >
        {TABS.map((t) => (
          <div
            key={t.name}
            className={`group relative h-8 flex items-center gap-2 px-3 text-[11.5px] rounded-t-md border-t border-l border-r mr-[2px] ${
              t.active ? "text-white/90" : "text-white/40 hover:text-white/65"
            }`}
            style={{
              background: t.active ? "#0B0B10" : "transparent",
              borderColor: t.active ? "rgba(255,255,255,0.07)" : "transparent",
            }}
          >
            <span className="font-mono">{t.name}</span>
            {t.dirty && <Circle className="w-1.5 h-1.5 fill-current" style={{ color: manual ? "#7E8190" : "#39D7FF" }} />}
            <X className="w-3 h-3 text-white/30 hover:text-white/70" />
            {t.active && (
              <span
                className="absolute -top-px left-0 right-0 h-[1px]"
                style={{ background: manual ? "rgba(126,129,144,0.5)" : "#39D7FF" }}
              />
            )}
          </div>
        ))}
      </div>

      {/* Breadcrumb */}
      <div className="h-7 shrink-0 flex items-center gap-1.5 px-3 border-b text-[11px] text-white/35" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
        <span>src</span>
        <ChevronRight className="w-3 h-3" />
        <span>{activeTab.path}</span>
        <ChevronRight className="w-3 h-3" />
        <span className="text-white/65">{activeTab.name}</span>
        <div className="ml-auto flex items-center gap-3 text-[10.5px]">
          <span className="flex items-center gap-1" style={{ color: "#4ADE80" }}>
            <Check className="w-3 h-3" /> tsc · 0 errors
          </span>
          {!manual && <span style={{ color: "#FFCC66" }}>⚡ 3 AI suggestions</span>}
          <span className="text-white/35">TS · LF · UTF-8</span>
        </div>
      </div>

      {/* Code area */}
      <div className="flex-1 min-h-0 flex overflow-hidden">
        <div className="flex-1 overflow-auto relative">
          <div className="font-mono text-[13px] leading-[20px] py-3">
            {CODE.map((l) => (
              <div
                key={l.n}
                className="group relative flex items-start hover:bg-white/[0.015]"
                style={l.selected ? { background: "rgba(75,140,255,0.10)", boxShadow: "inset 2px 0 0 rgba(75,140,255,0.6)" } : undefined}
              >
                <div className="w-12 shrink-0 text-right pr-3 text-white/25 select-none">{l.n}</div>
                <div className="w-1.5 shrink-0">
                  {l.gutter === "add" && <div className="w-[2px] h-full" style={{ background: "rgba(74,222,128,0.7)" }} />}
                  {l.gutter === "mod" && <div className="w-[2px] h-full" style={{ background: "rgba(57,215,255,0.7)" }} />}
                </div>
                <div className="flex-1 pr-6">
                  <span dangerouslySetInnerHTML={{ __html: l.html }} />
                  {l.ghost && (
                    <span className="text-white/22 italic select-none">{l.ghost}</span>
                  )}
                  {l.commentary && !manual && (
                    <div
                      className="mt-1 mb-2 inline-flex items-center gap-2 px-2 py-1 rounded-md text-[10.5px]"
                      style={{ background: "rgba(139,92,255,0.07)", border: "1px solid rgba(139,92,255,0.22)", color: "#C8B5FF" }}
                    >
                      <Sparkles className="w-3 h-3" />
                      {l.commentary}
                      <span className="ml-2 text-white/40">Accept</span>
                      <span className="text-white/25">·</span>
                      <span className="text-white/40">Reject</span>
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>

          {/* Assisted: contextual suggestion popover near selection */}
          {assisted && (
            <>
              <div
                className="absolute left-[260px] top-[170px] w-[280px] rounded-lg border overflow-hidden"
                style={{ background: "#15151F", borderColor: "rgba(75,140,255,0.30)", boxShadow: "0 12px 32px -12px rgba(0,0,0,0.5)" }}
              >
                <div
                  className="px-2.5 py-1.5 flex items-center justify-between border-b"
                  style={{ borderColor: "rgba(255,255,255,0.05)" }}
                >
                  <span className="flex items-center gap-1.5 text-[10.5px]" style={{ color: "#A8C3FF" }}>
                    <Sparkles className="w-3 h-3" /> Suggestions
                  </span>
                  <span className="text-[9.5px] text-white/35 font-mono">3 for selection</span>
                </div>
                <div className="py-1">
                  {[
                    { icon: Wand2, label: "Refactor validation into helper", k: "1" },
                    { icon: Lightbulb, label: "Add null-check for customerId", k: "2", emphasized: true },
                    { icon: TestTube2, label: "Generate unit test", k: "3" },
                    { icon: FileTextIcon, label: "Generate docstring", k: "4" },
                  ].map((s, i) => (
                    <div
                      key={i}
                      className="flex items-center gap-2 px-2.5 h-7 text-[11.5px] cursor-default"
                      style={{
                        background: s.emphasized ? "rgba(75,140,255,0.08)" : "transparent",
                        color: s.emphasized ? "#F4F4F6" : "rgba(244,244,246,0.75)",
                      }}
                    >
                      <s.icon className="w-3 h-3" style={{ color: s.emphasized ? "#4B8CFF" : "#7E8190" }} />
                      <span className="flex-1 truncate">{s.label}</span>
                      <span className="text-[9.5px] text-white/30 font-mono">⌥{s.k}</span>
                    </div>
                  ))}
                </div>
                {/* Diff preview overlay */}
                <div
                  className="px-2.5 py-2 border-t font-mono text-[10.5px] leading-[1.6]"
                  style={{ borderColor: "rgba(255,255,255,0.05)", background: "#101018" }}
                >
                  <div className="flex items-center justify-between text-[9.5px] text-white/40 mb-1">
                    <span>Preview · subscriptions.ts</span>
                    <span className="flex items-center gap-1">
                      <span style={{ color: "#4ADE80" }}>+2</span>
                      <span style={{ color: "#FF5C7A" }}>−0</span>
                    </span>
                  </div>
                  <div className="flex items-start gap-1.5" style={{ background: "rgba(74,222,128,0.07)" }}>
                    <Plus className="w-2.5 h-2.5 mt-1 shrink-0" style={{ color: "#4ADE80" }} />
                    <span
                      className="text-white/75 truncate"
                      dangerouslySetInnerHTML={{
                        __html: `<span class="tk-key">if</span> (!customerId) <span class="tk-key">throw new</span> <span class="tk-typ">Error</span>(<span class="tk-str">"missing_customer"</span>);`,
                      }}
                    />
                  </div>
                  <div className="flex items-start gap-1.5" style={{ background: "rgba(74,222,128,0.07)" }}>
                    <Plus className="w-2.5 h-2.5 mt-1 shrink-0" style={{ color: "#4ADE80" }} />
                    <span className="tk-cmt">// guard against null Stripe customer</span>
                  </div>
                </div>
                <div
                  className="px-2.5 py-1.5 flex items-center justify-between border-t text-[10px] text-white/45"
                  style={{ borderColor: "rgba(255,255,255,0.05)" }}
                >
                  <div className="flex items-center gap-2.5">
                    <span><span className="font-mono text-white/60">Tab</span> Accept</span>
                    <span><span className="font-mono text-white/60">Esc</span> Dismiss</span>
                    <span><span className="font-mono text-white/60">⌘↵</span> Expand</span>
                  </div>
                </div>
              </div>

              {/* Inline ghost-completion hint */}
              <div
                className="absolute right-6 top-3 flex items-center gap-1.5 px-2 py-1 rounded-md text-[10.5px]"
                style={{ background: "rgba(57,215,255,0.08)", border: "1px solid rgba(57,215,255,0.25)", color: "#9EE9FF" }}
              >
                <Wand2 className="w-3 h-3" /> Ghost completion · <span className="font-mono text-white/55">Tab</span> to accept
              </div>
            </>
          )}

          {/* Manual mode: tiny ghost-completion hint */}
          {manual && (
            <div
              className="absolute right-6 top-[480px] flex items-center gap-2 px-2 py-1 rounded-md text-[10.5px] backdrop-blur"
              style={{
                background: "rgba(21,21,31,0.9)",
                border: "1px solid rgba(255,255,255,0.07)",
                color: "#B6B7C3",
              }}
            >
              <span className="font-mono text-white/55">Tab</span>
              <span>to accept completion</span>
              <CornerDownLeft className="w-3 h-3 text-white/35" />
            </div>
          )}

          {/* Co-Pilot suggestion (hidden in Manual & Assisted) */}
          {!manual && !assisted && (
            <div
              className="absolute top-[148px] right-6 w-[260px] p-2.5 rounded-lg border"
              style={{ background: "#15151F", borderColor: "rgba(255,255,255,0.08)" }}
            >
              <div className="flex items-center gap-1.5 text-[10.5px]" style={{ color: "#C8B5FF" }}>
                <Sparkles className="w-3 h-3" /> Orion · Co-Pilot
              </div>
              <div className="mt-1.5 text-[11.5px] text-white/80 leading-snug">
                Parallelize task dispatch with bounded concurrency (8).
              </div>
              <div className="mt-2 flex items-center justify-between">
                <span className="text-[10px] text-white/35">+18 / −4 · 2 files</span>
                <div className="flex gap-1">
                  <button className="px-2 h-6 rounded-md text-[10.5px] text-white/55 bg-white/[0.04] border border-white/[0.07]">Diff</button>
                  <button
                    className="px-2 h-6 rounded-md text-[10.5px] font-medium"
                    style={{ color: "#09090D", background: "#39D7FF" }}
                  >
                    Apply
                  </button>
                </div>
              </div>
            </div>
          )}
        </div>

        {/* Minimap */}
        <div className="w-[56px] shrink-0 border-l py-2 px-1.5" style={{ borderColor: "rgba(255,255,255,0.04)" }}>
          <div className="h-full w-full rounded relative" style={{ background: "rgba(255,255,255,0.02)" }}>
            <div className="absolute left-1 right-1 top-[18%] h-[28%] rounded-sm" style={{ background: "rgba(255,255,255,0.07)", border: "1px solid rgba(255,255,255,0.08)" }} />
            {!manual && <div className="absolute left-1 right-1 top-[44%] h-[2px]" style={{ background: "rgba(74,222,128,0.6)" }} />}
            {!manual && <div className="absolute left-1 right-1 top-[48%] h-[2px]" style={{ background: "rgba(57,215,255,0.6)" }} />}
          </div>
        </div>
      </div>
    </div>
  );
}

/* ---------- Level 3 · Co-Pilot canvas ---------- */

const PLAN_STEPS = [
  { n: 1, label: "Inspect existing auth middleware", state: "done" },
  { n: 2, label: "Add token expiry validation", state: "active" },
  { n: 3, label: "Update user model", state: "queued" },
  { n: 4, label: "Generate tests", state: "queued" },
  { n: 5, label: "Run test suite", state: "queued" },
];

const EDITOR_LINES = [
  { n: 1, html: `<span class="tk-key">import</span> { <span class="tk-id">NextRequest</span>, <span class="tk-id">NextResponse</span> } <span class="tk-key">from</span> <span class="tk-str">"next/server"</span>;` },
  { n: 2, html: `<span class="tk-key">import</span> { <span class="tk-fn">verifyToken</span> } <span class="tk-key">from</span> <span class="tk-str">"@/lib/jwt"</span>;` },
  { n: 3, html: `&nbsp;` },
  { n: 4, html: `<span class="tk-key">export async function</span> <span class="tk-fn">middleware</span>(req: <span class="tk-typ">NextRequest</span>) {` },
  { n: 5, html: `&nbsp;&nbsp;<span class="tk-key">const</span> token = req.cookies.<span class="tk-fn">get</span>(<span class="tk-str">"nebula_session"</span>)?.value;` },
  { n: 6, html: `&nbsp;&nbsp;<span class="tk-key">if</span> (!token) <span class="tk-key">return</span> <span class="tk-typ">NextResponse</span>.<span class="tk-fn">redirect</span>(<span class="tk-str">"/login"</span>);` },
  { n: 7, html: `&nbsp;` },
  { n: 8, html: `&nbsp;&nbsp;<span class="tk-key">const</span> session = <span class="tk-key">await</span> <span class="tk-fn">verifyToken</span>(token);` },
  { n: 9, html: `&nbsp;&nbsp;<span class="tk-key">if</span> (!session) <span class="tk-key">return</span> <span class="tk-typ">NextResponse</span>.<span class="tk-fn">redirect</span>(<span class="tk-str">"/login"</span>);` },
  { n: 10, html: `&nbsp;` },
  { n: 11, html: `&nbsp;&nbsp;req.headers.<span class="tk-fn">set</span>(<span class="tk-str">"x-user-id"</span>, session.sub);` },
  { n: 12, html: `&nbsp;&nbsp;<span class="tk-key">return</span> <span class="tk-typ">NextResponse</span>.<span class="tk-fn">next</span>();` },
  { n: 13, html: `}` },
];

const DIFF_FILES = [
  {
    name: "auth/middleware.ts",
    plus: 6,
    minus: 1,
    annotation: { who: "GPT-5.5", body: "Add token expiration validation; redirect on expired session." },
    annotationColor: "#4B8CFF",
    hunks: [
      { kind: "ctx" as const, n: 8, html: `<span class="tk-key">const</span> session = <span class="tk-key">await</span> <span class="tk-fn">verifyToken</span>(token);` },
      { kind: "rem" as const, n: 9, html: `<span class="tk-key">if</span> (!session) <span class="tk-key">return</span> <span class="tk-typ">NextResponse</span>.<span class="tk-fn">redirect</span>(<span class="tk-str">"/login"</span>);` },
      { kind: "add" as const, n: 9, html: `<span class="tk-key">if</span> (!session || <span class="tk-fn">isExpired</span>(session.exp)) {` },
      { kind: "add" as const, n: 10, html: `&nbsp;&nbsp;<span class="tk-key">return</span> <span class="tk-typ">NextResponse</span>.<span class="tk-fn">redirect</span>(<span class="tk-str">"/login?reason=expired"</span>);` },
      { kind: "add" as const, n: 11, html: `}` },
      { kind: "ctx" as const, n: 12, html: `req.headers.<span class="tk-fn">set</span>(<span class="tk-str">"x-user-id"</span>, session.sub);` },
    ],
  },
  {
    name: "models/user.ts",
    plus: 3,
    minus: 0,
    annotation: { who: "Claude", body: "Extract provider-specific subscription logic from User into SubscriptionProvider." },
    annotationColor: "#8B5CFF",
    hunks: [
      { kind: "ctx" as const, n: 18, html: `<span class="tk-key">export type</span> <span class="tk-typ">UserRole</span> = <span class="tk-str">"admin"</span> | <span class="tk-str">"editor"</span> | <span class="tk-str">"viewer"</span>;` },
      { kind: "add" as const, n: 19, html: `<span class="tk-key">export interface</span> <span class="tk-typ">SessionClaims</span> {` },
      { kind: "add" as const, n: 20, html: `&nbsp;&nbsp;sub: <span class="tk-typ">string</span>; role: <span class="tk-typ">UserRole</span>; exp: <span class="tk-typ">number</span>;` },
      { kind: "add" as const, n: 21, html: `}` },
    ],
  },
  {
    name: "tests/auth.test.ts",
    plus: 22,
    minus: 0,
    annotation: { who: "Local", body: "Generated cases for expired token, missing token, and tampered signature." },
    annotationColor: "#39D7FF",
    hunks: [
      { kind: "add" as const, n: 1, html: `<span class="tk-key">it</span>(<span class="tk-str">"redirects expired sessions to /login?reason=expired"</span>, <span class="tk-key">async</span> () =&gt; {` },
      { kind: "add" as const, n: 2, html: `&nbsp;&nbsp;<span class="tk-key">const</span> res = <span class="tk-key">await</span> <span class="tk-fn">middleware</span>(<span class="tk-fn">withCookie</span>(<span class="tk-fn">expired</span>()));` },
      { kind: "add" as const, n: 3, html: `&nbsp;&nbsp;<span class="tk-fn">expect</span>(res.status).<span class="tk-fn">toBe</span>(<span class="tk-num">307</span>);` },
      { kind: "add" as const, n: 4, html: `});` },
    ],
  },
];

function CopilotCanvas() {
  return (
    <div className="flex-1 min-w-0 h-full flex flex-col" style={{ background: "#0B0B10" }}>
      {/* Co-Pilot Plan strip */}
      <div
        className="shrink-0 border-b px-3 py-2 flex items-center gap-2"
        style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-1.5 pr-3 border-r" style={{ borderColor: "rgba(255,255,255,0.06)" }}>
          <Sparkles className="w-3.5 h-3.5" style={{ color: "#8B5CFF" }} />
          <span className="text-[10.5px] uppercase tracking-[0.16em] text-white/45">Co-Pilot Plan</span>
        </div>
        <div className="flex items-center gap-1 flex-1 min-w-0">
          {PLAN_STEPS.map((s, i) => {
            const done = s.state === "done";
            const active = s.state === "active";
            return (
              <div key={s.n} className="flex items-center gap-1 min-w-0">
                <div
                  className="flex items-center gap-1.5 h-6 px-2 rounded-md text-[11px]"
                  style={{
                    background: active ? "rgba(75,140,255,0.12)" : done ? "rgba(74,222,128,0.06)" : "rgba(255,255,255,0.03)",
                    border: `1px solid ${active ? "rgba(75,140,255,0.35)" : done ? "rgba(74,222,128,0.22)" : "rgba(255,255,255,0.06)"}`,
                    color: active ? "#A8C3FF" : done ? "#86EFAC" : "rgba(255,255,255,0.55)",
                  }}
                >
                  {done ? (
                    <Check className="w-3 h-3" />
                  ) : (
                    <span className="font-mono text-[9.5px] opacity-70">{s.n}</span>
                  )}
                  <span className="truncate">{s.label}</span>
                </div>
                {i < PLAN_STEPS.length - 1 && <ChevronRight className="w-3 h-3 text-white/25 shrink-0" />}
              </div>
            );
          })}
        </div>
        <div className="flex items-center gap-1 pl-2">
          <button
            className="h-6 px-2 rounded-md text-[10.5px] text-white/65 border"
            style={{ background: "rgba(255,255,255,0.03)", borderColor: "rgba(255,255,255,0.07)" }}
          >
            Revise plan
          </button>
        </div>
      </div>

      <div className="flex-1 min-h-0 flex">
        {/* Editor pane */}
        <div className="w-1/2 min-w-0 flex flex-col border-r" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
          <div
            className="h-8 shrink-0 flex items-center gap-2 px-3 border-b text-[11px]"
            style={{ background: "#0D0D12", borderColor: "rgba(255,255,255,0.05)" }}
          >
            <FileTextIcon className="w-3 h-3 text-white/45" />
            <span className="font-mono text-white/80">auth/middleware.ts</span>
            <Circle className="w-1.5 h-1.5 fill-current" style={{ color: "#4B8CFF" }} />
            <span className="ml-auto text-[10px] text-white/35">current</span>
          </div>
          <div className="flex-1 overflow-auto font-mono text-[13px] leading-[20px] py-2">
            {EDITOR_LINES.map((l) => (
              <div key={l.n} className="flex items-start hover:bg-white/[0.015]">
                <div className="w-10 shrink-0 text-right pr-3 text-white/25 select-none">{l.n}</div>
                <div className="w-1.5 shrink-0" />
                <div className="flex-1 pr-6" dangerouslySetInnerHTML={{ __html: l.html }} />
              </div>
            ))}
          </div>
        </div>

        {/* Diff pane */}
        <div className="w-1/2 min-w-0 flex flex-col" style={{ background: "#0D0D12" }}>
          <div
            className="h-8 shrink-0 flex items-center justify-between px-3 border-b text-[11px]"
            style={{ borderColor: "rgba(255,255,255,0.05)" }}
          >
            <div className="flex items-center gap-2">
              <Sparkles className="w-3 h-3" style={{ color: "#8B5CFF" }} />
              <span className="text-white/85 font-medium">Proposed changes</span>
              <span className="text-white/40">· 3 files</span>
              <span className="text-[10.5px]" style={{ color: "#4ADE80" }}>+31</span>
              <span className="text-[10.5px]" style={{ color: "#FF5C7A" }}>−1</span>
            </div>
            <div className="flex items-center gap-1">
              <button className="h-6 px-2 rounded-md text-[10.5px] text-white/65 border" style={{ background: "rgba(255,255,255,0.03)", borderColor: "rgba(255,255,255,0.07)" }}>
                Request revision
              </button>
              <button
                className="h-6 px-2.5 rounded-md text-[10.5px] font-semibold"
                style={{ color: "#09090D", background: "linear-gradient(135deg,#39D7FF 0%,#8B5CFF 100%)" }}
              >
                Apply all
              </button>
            </div>
          </div>

          <div className="flex-1 overflow-auto">
            {DIFF_FILES.map((f) => (
              <div key={f.name} className="border-b" style={{ borderColor: "rgba(255,255,255,0.05)" }}>
                <div
                  className="flex items-center justify-between px-3 h-8 sticky top-0"
                  style={{ background: "#15151F" }}
                >
                  <div className="flex items-center gap-2 min-w-0">
                    <ChevronDown className="w-3 h-3 text-white/40" />
                    <FileTextIcon className="w-3 h-3 text-white/45" />
                    <span className="font-mono text-[11px] text-white/85 truncate">{f.name}</span>
                  </div>
                  <div className="flex items-center gap-1.5">
                    <span className="text-[10.5px] font-mono" style={{ color: "#4ADE80" }}>+{f.plus}</span>
                    <span className="text-[10.5px] font-mono" style={{ color: "#FF5C7A" }}>−{f.minus}</span>
                    <button className="h-5 px-1.5 rounded text-[10px] text-white/55 hover:bg-white/[0.05]">
                      Reject
                    </button>
                    <button
                      className="h-5 px-1.5 rounded text-[10px]"
                      style={{ background: "rgba(74,222,128,0.12)", color: "#86EFAC", border: "1px solid rgba(74,222,128,0.25)" }}
                    >
                      Accept
                    </button>
                  </div>
                </div>

                {/* Annotation */}
                <div
                  className="px-3 py-1.5 flex items-start gap-2 border-b text-[10.5px]"
                  style={{ borderColor: "rgba(255,255,255,0.04)" }}
                >
                  <Sparkles className="w-3 h-3 mt-[2px] shrink-0" style={{ color: f.annotationColor }} />
                  <span className="text-white/75">
                    <span style={{ color: f.annotationColor }} className="font-medium">{f.annotation.who}</span>{" "}
                    <span className="text-white/55">{f.annotation.body}</span>
                  </span>
                </div>

                {/* Hunks */}
                <div className="font-mono text-[12px] leading-[18px] py-1">
                  {f.hunks.map((h, i) => (
                    <div
                      key={i}
                      className="flex items-start px-3"
                      style={{
                        background:
                          h.kind === "add"
                            ? "rgba(74,222,128,0.08)"
                            : h.kind === "rem"
                            ? "rgba(255,92,122,0.08)"
                            : "transparent",
                      }}
                    >
                      <span className="w-6 text-right pr-2 text-white/25 select-none">{h.n}</span>
                      <span
                        className="w-3 text-center select-none"
                        style={{
                          color: h.kind === "add" ? "#4ADE80" : h.kind === "rem" ? "#FF5C7A" : "rgba(255,255,255,0.2)",
                        }}
                      >
                        {h.kind === "add" ? "+" : h.kind === "rem" ? "−" : " "}
                      </span>
                      <span className="flex-1 pl-2" dangerouslySetInnerHTML={{ __html: h.html }} />
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

const DELEGATED_DIFF: { n: number; t: "ctx" | "add" | "del"; html: string }[] = [
  { n: 41, t: "ctx", html: `<span class="tk-key">export async function</span> <span class="tk-fn">handleSubscriptionUpdated</span>(<span class="tk-id">event</span>: <span class="tk-typ">Stripe.Event</span>) {` },
  { n: 42, t: "ctx", html: `  <span class="tk-key">const</span> <span class="tk-id">sub</span> = <span class="tk-id">event.data.object</span> <span class="tk-key">as</span> <span class="tk-typ">Stripe.Subscription</span>;` },
  { n: 43, t: "del", html: `  <span class="tk-key">await</span> <span class="tk-id">db.subscriptions</span>.<span class="tk-fn">update</span>({ <span class="tk-id">where</span>: { <span class="tk-id">id</span>: <span class="tk-id">sub.id</span> }, <span class="tk-id">data</span>: { <span class="tk-id">status</span>: <span class="tk-id">sub.status</span> } });` },
  { n: 44, t: "add", html: `  <span class="tk-key">const</span> <span class="tk-id">existing</span> = <span class="tk-key">await</span> <span class="tk-id">db.subscriptions</span>.<span class="tk-fn">findUnique</span>({ <span class="tk-id">where</span>: { <span class="tk-id">id</span>: <span class="tk-id">sub.id</span> } });` },
  { n: 45, t: "add", html: `  <span class="tk-key">if</span> (!<span class="tk-id">existing</span>) <span class="tk-key">throw new</span> <span class="tk-typ">SubscriptionNotFoundError</span>(<span class="tk-id">sub.id</span>);` },
  { n: 46, t: "add", html: `  <span class="tk-key">if</span> (<span class="tk-id">existing.eventVersion</span> &gt;= <span class="tk-id">event.id</span>) <span class="tk-key">return</span>; <span class="tk-cmt">// idempotency guard</span>` },
  { n: 47, t: "add", html: `  <span class="tk-key">await</span> <span class="tk-id">db.subscriptions</span>.<span class="tk-fn">update</span>({` },
  { n: 48, t: "add", html: `    <span class="tk-id">where</span>: { <span class="tk-id">id</span>: <span class="tk-id">sub.id</span> },` },
  { n: 49, t: "add", html: `    <span class="tk-id">data</span>: { <span class="tk-id">status</span>: <span class="tk-id">sub.status</span>, <span class="tk-id">eventVersion</span>: <span class="tk-id">event.id</span>, <span class="tk-id">updatedAt</span>: <span class="tk-key">new</span> <span class="tk-typ">Date</span>() },` },
  { n: 50, t: "add", html: `  });` },
  { n: 51, t: "ctx", html: `  <span class="tk-key">await</span> <span class="tk-fn">notifyBilling</span>(<span class="tk-id">sub.customer</span>);` },
  { n: 52, t: "ctx", html: `}` },
];

const COLUMNS: { key: string; title: string; tone: string; bg: string; border: string; cards: any[] }[] = [
  {
    key: "assigned", title: "Assigned", tone: "#7E8190", bg: "rgba(126,129,144,0.08)", border: "rgba(126,129,144,0.25)",
    cards: [
      { title: "Audit auth middleware side effects", agent: "Review", agentColor: "#8B5CFF", risk: "Low", riskColor: "#4ADE80", files: 2, progress: 0, test: "—" },
    ],
  },
  {
    key: "progress", title: "In Progress", tone: "#4B8CFF", bg: "rgba(75,140,255,0.08)", border: "rgba(75,140,255,0.30)",
    cards: [
      { title: "Wire Stripe webhook endpoint", agent: "Backend", agentColor: "#4B8CFF", risk: "Medium", riskColor: "#FFCC66", files: 4, progress: 62, test: "8/14" },
      { title: "Implement billing settings page", agent: "Frontend", agentColor: "#39D7FF", risk: "Low", riskColor: "#4ADE80", files: 3, progress: 78, test: "—" },
    ],
  },
  {
    key: "human", title: "Waiting on Human", tone: "#FFB86B", bg: "rgba(255,184,107,0.10)", border: "rgba(255,184,107,0.35)",
    cards: [
      { title: "Refactor user billing schema", agent: "Backend", agentColor: "#4B8CFF", risk: "High", riskColor: "#FF5C7A", files: 6, progress: 100, test: "passed", waiting: "Approve migration" },
    ],
  },
  {
    key: "testing", title: "Testing", tone: "#39D7FF", bg: "rgba(57,215,255,0.08)", border: "rgba(57,215,255,0.30)",
    cards: [
      { title: "Generate subscription lifecycle tests", agent: "QA", agentColor: "#39D7FF", risk: "Low", riskColor: "#4ADE80", files: 2, progress: 85, test: "12/14" },
    ],
  },
  {
    key: "done", title: "Done", tone: "#4ADE80", bg: "rgba(74,222,128,0.08)", border: "rgba(74,222,128,0.25)",
    cards: [
      { title: "Add Stripe SDK dependency", agent: "Backend", agentColor: "#4B8CFF", risk: "Low", riskColor: "#4ADE80", files: 1, progress: 100, test: "passed" },
      { title: "Scaffold /billing route", agent: "Frontend", agentColor: "#39D7FF", risk: "Low", riskColor: "#4ADE80", files: 2, progress: 100, test: "passed" },
    ],
  },
];

function DelegatedCanvas() {
  return (
    <div className="flex-1 min-w-0 h-full flex flex-col" style={{ background: "#0B0B10" }}>
      {/* Top: Diff Review */}
      <div className="flex flex-col" style={{ flex: "1 1 60%", minHeight: 0 }}>
        {/* Header */}
        <div
          className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
          style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
        >
          <div className="flex items-center gap-2 text-[11.5px]">
            <FileTextIcon className="w-3.5 h-3.5 text-white/45" />
            <span className="font-mono text-white/85">api/billing/webhook.ts</span>
            <span
              className="ml-1 px-1.5 py-[1px] rounded text-[10px] font-mono"
              style={{ color: "#4ADE80", background: "rgba(74,222,128,0.08)", border: "1px solid rgba(74,222,128,0.25)" }}
            >
              +8 / −1
            </span>
            <span
              className="px-1.5 py-[1px] rounded text-[10px]"
              style={{ color: "#FFB86B", background: "rgba(255,184,107,0.08)", border: "1px solid rgba(255,184,107,0.25)" }}
            >
              Awaiting approval
            </span>
          </div>
          <div className="flex items-center gap-1">
            <button
              className="h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] text-white/70 bg-white/[0.04] border border-white/[0.08] hover:bg-white/[0.06]"
            >
              <FileTextIcon className="w-3 h-3" /> Open Task
            </button>
            <button
              className="h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] text-white/70 bg-white/[0.04] border border-white/[0.08] hover:bg-white/[0.06]"
            >
              <TestTube2 className="w-3 h-3" /> Run Tests
            </button>
            <button
              className="h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] text-rose-300 bg-white/[0.03] border border-white/[0.08] hover:bg-rose-400/10"
            >
              <X className="w-3 h-3" /> Request Changes
            </button>
            <button
              className="h-7 px-3 flex items-center gap-1.5 rounded-md text-[11px] font-semibold"
              style={{ color: "#09090D", background: "linear-gradient(135deg, #39D7FF 0%, #8B5CFF 100%)" }}
            >
              <Check className="w-3 h-3" /> Approve
            </button>
          </div>
        </div>

        {/* Annotation */}
        <div
          className="px-3 py-2 flex items-center justify-between border-b text-[11px]"
          style={{ background: "#101018", borderColor: "rgba(255,255,255,0.05)" }}
        >
          <div className="flex items-center gap-2">
            <span
              className="w-5 h-5 rounded-md grid place-items-center text-[10px] font-mono text-white/85"
              style={{ background: "rgba(75,140,255,0.15)", border: "1px solid rgba(75,140,255,0.35)", color: "#A8C3FF" }}
            >
              B
            </span>
            <span className="text-white/65">
              <span className="text-white/90 font-medium">Backend Agent</span> modified subscription controller — added idempotency guard + event versioning
            </span>
          </div>
          <div className="flex items-center gap-2 text-[10.5px]">
            <span className="text-white/40">commit</span>
            <span className="font-mono text-white/65">a4f1c2e</span>
            <span className="text-white/30">·</span>
            <span className="text-white/40">42s ago</span>
          </div>
        </div>

        {/* Test status banner */}
        <div
          className="px-3 py-1.5 flex items-center justify-between border-b text-[11px]"
          style={{ background: "rgba(255,184,107,0.06)", borderColor: "rgba(255,184,107,0.18)" }}
        >
          <div className="flex items-center gap-2">
            <TestTube2 className="w-3.5 h-3.5" style={{ color: "#FFB86B" }} />
            <span className="text-white/80">
              <span className="font-mono">12/14</span> tests passing
            </span>
            <span className="text-white/40">·</span>
            <span style={{ color: "#FF8FA3" }}>2 failures under review</span>
          </div>
          <button className="text-[10.5px] text-white/55 hover:text-white/85 underline decoration-white/20 underline-offset-2">
            View failures
          </button>
        </div>

        {/* Diff */}
        <div className="flex-1 overflow-auto">
          <div className="font-mono text-[12.5px] leading-[19px] py-2">
            {DELEGATED_DIFF.map((l, i) => {
              const bg =
                l.t === "add" ? "rgba(74,222,128,0.07)" :
                l.t === "del" ? "rgba(255,92,122,0.07)" : "transparent";
              const mark = l.t === "add" ? "+" : l.t === "del" ? "−" : " ";
              const markColor = l.t === "add" ? "#4ADE80" : l.t === "del" ? "#FF5C7A" : "rgba(255,255,255,0.25)";
              return (
                <div key={i} className="flex items-start" style={{ background: bg }}>
                  <div className="w-10 shrink-0 text-right pr-2 text-white/25 select-none">{l.n}</div>
                  <div className="w-4 shrink-0 text-center select-none" style={{ color: markColor }}>{mark}</div>
                  <div className="flex-1 pr-6">
                    <span dangerouslySetInnerHTML={{ __html: l.html }} />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      </div>

      {/* Bottom: Task Board */}
      <div
        className="flex flex-col border-t shrink-0"
        style={{ flex: "1 1 40%", minHeight: 0, background: "#0D0D12", borderColor: "rgba(255,255,255,0.06)" }}
      >
        <div
          className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
          style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
        >
          <div className="flex items-center gap-2">
            <Sparkles className="w-3.5 h-3.5" style={{ color: "#8B5CFF" }} />
            <span className="text-[12px] text-white/90 font-medium tracking-tight">Delegated Task Board</span>
            <span className="text-[10px] text-white/40">· 7 active</span>
          </div>
          <div className="flex items-center gap-2 text-[10.5px] text-white/40">
            <span className="font-mono">sprint · stripe-subscriptions</span>
            <ChevronDown className="w-3 h-3" />
          </div>
        </div>

        <div className="flex-1 min-h-0 overflow-x-auto">
          <div className="flex gap-2 px-2 py-2 min-w-max h-full">
            {COLUMNS.map((col) => (
              <div
                key={col.key}
                className="w-[230px] shrink-0 flex flex-col rounded-lg border"
                style={{ background: "#0F0F17", borderColor: "rgba(255,255,255,0.05)" }}
              >
                <div
                  className="h-7 shrink-0 flex items-center justify-between px-2.5 border-b"
                  style={{ borderColor: "rgba(255,255,255,0.05)" }}
                >
                  <div className="flex items-center gap-1.5">
                    <span className="w-1.5 h-1.5 rounded-full" style={{ background: col.tone }} />
                    <span className="text-[10.5px] uppercase tracking-[0.14em] text-white/65 font-medium">{col.title}</span>
                  </div>
                  <span
                    className="text-[10px] px-1 rounded font-mono"
                    style={{ color: col.tone, background: col.bg, border: `1px solid ${col.border}` }}
                  >
                    {col.cards.length}
                  </span>
                </div>

                <div className="flex-1 overflow-y-auto p-1.5 space-y-1.5">
                  {col.cards.map((c, i) => (
                    <div
                      key={i}
                      className="p-2 rounded-md border"
                      style={{
                        background: col.key === "human" ? "rgba(255,184,107,0.06)" : "#15151F",
                        borderColor: col.key === "human" ? "rgba(255,184,107,0.30)" : "rgba(255,255,255,0.06)",
                      }}
                    >
                      <div className="text-[11.5px] text-white/90 leading-snug mb-1.5">{c.title}</div>
                      <div className="flex items-center gap-1.5 mb-1.5">
                        <span
                          className="w-4 h-4 rounded grid place-items-center text-[9px] font-mono"
                          style={{ background: `${c.agentColor}1F`, color: c.agentColor, border: `1px solid ${c.agentColor}55` }}
                        >
                          {c.agent[0]}
                        </span>
                        <span className="text-[10px] text-white/55">{c.agent}</span>
                        <span className="ml-auto text-[9.5px]" style={{ color: c.riskColor }}>{c.risk}</span>
                      </div>
                      <div
                        className="h-[2px] w-full rounded-full overflow-hidden"
                        style={{ background: "rgba(255,255,255,0.05)" }}
                      >
                        <div
                          className="h-full rounded-full"
                          style={{ width: `${c.progress}%`, background: col.tone, opacity: 0.85 }}
                        />
                      </div>
                      <div className="mt-1.5 flex items-center justify-between text-[10px] text-white/45 font-mono">
                        <span>{c.files} file{c.files === 1 ? "" : "s"}</span>
                        <span style={{ color: c.test === "passed" ? "#4ADE80" : c.test === "—" ? "rgba(255,255,255,0.3)" : "#9EE9FF" }}>
                          {c.test === "passed" ? "✓ passed" : c.test === "—" ? "—" : `tests ${c.test}`}
                        </span>
                      </div>
                      {c.waiting && (
                        <div
                          className="mt-1.5 px-1.5 py-1 rounded text-[10px] flex items-center gap-1.5"
                          style={{ background: "rgba(255,184,107,0.12)", color: "#FFB86B", border: "1px solid rgba(255,184,107,0.30)" }}
                        >
                          <Lightbulb className="w-3 h-3" />
                          {c.waiting}
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

const FLEET_COLUMNS = [
  {
    key: "breakdown", title: "Directive Breakdown", tone: "#7E8190", bg: "rgba(126,129,144,0.08)", border: "rgba(126,129,144,0.25)",
    cards: [
      { title: "Map existing user billing model", agent: "Planner", agentColor: "#8B5CFF", model: "Claude", risk: "Low", riskColor: "#4ADE80", files: 2, progress: 10, test: "—", time: "2m ago" },
      { title: "Identify Stripe integration points", agent: "Architect", agentColor: "#39D7FF", model: "GPT-5.5", risk: "Medium", riskColor: "#FFCC66", files: 5, progress: 5, test: "—", time: "1m ago" },
    ],
  },
  {
    key: "planning", title: "Agent Planning", tone: "#FFCC66", bg: "rgba(255,204,102,0.08)", border: "rgba(255,204,102,0.30)",
    cards: [
      { title: "Plan webhook event lifecycle", agent: "Backend", agentColor: "#FFB86B", model: "GPT-5.5", risk: "Medium", riskColor: "#FFCC66", files: 1, progress: 40, test: "—", time: "Just now" },
      { title: "Design subscription state transitions", agent: "Planner", agentColor: "#8B5CFF", model: "Claude", risk: "High", riskColor: "#FF5C7A", files: 3, progress: 65, test: "—", time: "3m ago", miniDiff: "+12 / −0 (user.ts)" },
    ],
  },
  {
    key: "executing", title: "Executing", tone: "#4B8CFF", bg: "rgba(75,140,255,0.08)", border: "rgba(75,140,255,0.30)",
    cards: [
      { title: "Implement checkout session endpoint", agent: "Backend", agentColor: "#4B8CFF", model: "GPT-5.5", risk: "Medium", riskColor: "#FFCC66", files: 2, progress: 85, test: "running", time: "12s ago", miniDiff: "+45 / −2" },
      { title: "Update user subscription schema", agent: "DB", agentColor: "#39D7FF", model: "Local", risk: "High", riskColor: "#FF5C7A", files: 4, progress: 95, test: "passed", time: "45s ago" },
      { title: "Refactor auth middleware for billing gates", agent: "Security", agentColor: "#8B5CFF", model: "Claude", risk: "High", riskColor: "#FF5C7A", files: 1, progress: 60, test: "failed", time: "1m ago", miniDiff: "+8 / −3 (middleware.ts)" },
    ],
  },
  {
    key: "testing", title: "Testing / Review", tone: "#8B5CFF", bg: "rgba(139,92,255,0.08)", border: "rgba(139,92,255,0.30)",
    cards: [
      { title: "Generate billing lifecycle test suite", agent: "QA", agentColor: "#39D7FF", model: "Gemini", risk: "Low", riskColor: "#4ADE80", files: 3, progress: 90, test: "14/14", time: "30s ago" },
      { title: "Review webhook signature validation", agent: "Review", agentColor: "#8B5CFF", model: "Claude", risk: "Medium", riskColor: "#FFCC66", files: 1, progress: 100, test: "passed", time: "2m ago", statusChip: "Verified" },
      { title: "Run regression tests for auth flow", agent: "QA", agentColor: "#39D7FF", model: "Local", risk: "Low", riskColor: "#4ADE80", files: 12, progress: 80, test: "running", time: "10s ago" },
    ],
  },
  {
    key: "done", title: "Completed", tone: "#4ADE80", bg: "rgba(74,222,128,0.08)", border: "rgba(74,222,128,0.25)",
    cards: [
      { title: "Install Stripe SDK", agent: "DevOps", agentColor: "#7E8190", model: "Local", risk: "Low", riskColor: "#4ADE80", files: 2, progress: 100, test: "passed", time: "10m ago", statusChip: "Merged" },
      { title: "Add environment variable schema", agent: "Backend", agentColor: "#4B8CFF", model: "GPT-5.5", risk: "Low", riskColor: "#4ADE80", files: 1, progress: 100, test: "passed", time: "9m ago", statusChip: "Merged" },
      { title: "Create billing service skeleton", agent: "Architect", agentColor: "#39D7FF", model: "Claude", risk: "Low", riskColor: "#4ADE80", files: 3, progress: 100, test: "passed", time: "8m ago", statusChip: "Merged" },
    ],
  },
];

function FleetCanvas() {
  return (
    <div className="flex-1 min-w-0 h-full flex flex-col" style={{ background: "#0B0B10" }}>
      {/* Top: Master Directive */}
      <div className="shrink-0 flex flex-col border-b" style={{ borderColor: "rgba(255,255,255,0.06)", background: "#111118" }}>
        <div className="h-12 px-4 flex items-center justify-between">
          <div className="flex items-center gap-3 flex-1 min-w-0">
            <div className="w-8 h-8 rounded-lg grid place-items-center shrink-0" style={{ background: "rgba(177,108,255,0.12)", border: "1px solid rgba(177,108,255,0.3)" }}>
              <Wand2 className="w-4 h-4" style={{ color: "#B16CFF" }} />
            </div>
            <div className="flex-1 min-w-0">
              <div className="text-[10px] uppercase tracking-[0.16em] text-[#B16CFF] mb-0.5 font-medium flex items-center gap-1.5">
                <span className="w-1.5 h-1.5 rounded-full bg-[#B16CFF] animate-pulse" />
                Master Directive
              </div>
              <div className="text-[14px] text-white/95 truncate">
                Implement Stripe subscriptions and wire them into the existing user model
              </div>
            </div>
          </div>
          <div className="flex items-center gap-2 pl-4 shrink-0">
            <button className="h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] text-white/60 bg-white/[0.03] border border-white/[0.08] hover:bg-white/[0.06]">
              Change Scope
            </button>
            <button className="h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] text-white/60 bg-white/[0.03] border border-white/[0.08] hover:bg-white/[0.06]">
              <Plus className="w-3 h-3" /> Add Constraint
            </button>
            <button className="h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] text-[#FFB86B] bg-white/[0.03] border border-white/[0.08] hover:bg-white/[0.06]">
              Force Review
            </button>
            <button className="h-7 px-2.5 flex items-center gap-1.5 rounded-md text-[11px] font-medium text-white/90 bg-white/10 hover:bg-white/15" style={{ border: "1px solid rgba(255,255,255,0.1)" }}>
              <span className="w-2 h-2 bg-[#FF5C7A] rounded-sm" /> Pause
            </button>
          </div>
        </div>
        
        {/* Metadata bar */}
        <div className="h-8 px-4 flex items-center gap-4 text-[10.5px] border-t" style={{ borderColor: "rgba(255,255,255,0.03)", background: "#0D0D12" }}>
          <span className="text-white/40">Started 12m ago</span>
          <span className="text-white/30">|</span>
          <span className="text-white/70"><span style={{ color: "#B16CFF" }}>9</span> agents active</span>
          <span className="text-white/30">|</span>
          <span className="text-white/70"><span style={{ color: "#4B8CFF" }}>27</span> files analyzed</span>
          <span className="text-white/30">|</span>
          <span className="text-white/70"><span style={{ color: "#FFCC66" }}>14</span> files modified</span>
          <span className="text-white/30">|</span>
          <div className="flex items-center gap-1.5 ml-auto">
            <span className="text-white/40">Confidence</span>
            <div className="flex items-center gap-1.5">
              <span className="text-white/90 font-mono">87%</span>
              <div className="w-16 h-1.5 rounded-full bg-white/10 overflow-hidden">
                <div className="h-full rounded-full" style={{ width: "87%", background: "#4ADE80" }} />
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* Kanban Board */}
      <div className="flex-1 min-h-0 overflow-x-auto p-3">
        <div className="flex gap-3 min-w-max h-full">
          {FLEET_COLUMNS.map((col) => (
            <div
              key={col.key}
              className="w-[280px] shrink-0 flex flex-col rounded-lg border shadow-sm"
              style={{ background: "#0F0F17", borderColor: "rgba(255,255,255,0.06)" }}
            >
              <div
                className="h-9 shrink-0 flex items-center justify-between px-3 border-b"
                style={{ borderColor: "rgba(255,255,255,0.06)", background: "rgba(255,255,255,0.01)" }}
              >
                <div className="flex items-center gap-2">
                  <span className="w-2 h-2 rounded-sm" style={{ background: col.tone }} />
                  <span className="text-[11px] uppercase tracking-[0.1em] text-white/80 font-medium">{col.title}</span>
                </div>
                <span
                  className="text-[10px] px-1.5 py-0.5 rounded font-mono"
                  style={{ color: col.tone, background: col.bg, border: `1px solid ${col.border}` }}
                >
                  {col.cards.length}
                </span>
              </div>

              <div className="flex-1 overflow-y-auto p-2.5 space-y-2.5">
                {col.cards.map((c, i) => (
                  <div
                    key={i}
                    className="p-2.5 rounded-lg border group relative hover:border-white/20 transition-colors"
                    style={{
                      background: "#15151F",
                      borderColor: "rgba(255,255,255,0.08)",
                      boxShadow: "0 2px 8px -2px rgba(0,0,0,0.4)"
                    }}
                  >
                    {/* Header: Status / Risk */}
                    <div className="flex items-center justify-between mb-1.5">
                      <div className="flex items-center gap-1.5">
                        <span
                          className="w-4 h-4 rounded grid place-items-center text-[9px] font-mono"
                          style={{ background: `${c.agentColor}1F`, color: c.agentColor, border: `1px solid ${c.agentColor}55` }}
                        >
                          {c.agent[0]}
                        </span>
                        <span className="text-[10px] text-white/70 font-medium">{c.agent}</span>
                        <span className="px-1 py-[1px] rounded text-[8.5px] font-mono text-white/40 bg-white/[0.04]">
                          {c.model}
                        </span>
                      </div>
                      <div className="flex items-center gap-1.5">
                        {c.statusChip && (
                          <span className="px-1.5 py-[1px] rounded text-[8px] uppercase tracking-wider font-medium" style={{ color: col.tone, border: `1px solid ${col.tone}40`, background: `${col.tone}10` }}>
                            {c.statusChip}
                          </span>
                        )}
                        <span className="text-[9px] uppercase tracking-wider" style={{ color: c.riskColor }}>{c.risk} RISK</span>
                      </div>
                    </div>

                    <div className="text-[12px] text-white/95 leading-snug mb-2.5 font-medium">{c.title}</div>
                    
                    {/* Mini diff / code */}
                    {c.miniDiff && (
                      <div className="mb-2 px-2 py-1.5 rounded bg-black/40 border border-white/5 text-[10px] font-mono text-white/55 flex items-center gap-1.5">
                        <FileTextIcon className="w-3 h-3 text-white/30" />
                        <span className="truncate">{c.miniDiff}</span>
                      </div>
                    )}

                    <div
                      className="h-[3px] w-full rounded-full overflow-hidden mb-2"
                      style={{ background: "rgba(255,255,255,0.06)" }}
                    >
                      <div
                        className="h-full rounded-full"
                        style={{ width: `${c.progress}%`, background: col.tone, opacity: 0.9 }}
                      />
                    </div>
                    
                    <div className="flex items-center justify-between text-[10px] text-white/40 font-mono">
                      <div className="flex items-center gap-2">
                        <span>{c.files} file{c.files === 1 ? "" : "s"}</span>
                        <span>·</span>
                        <span style={{ color: c.test === "passed" ? "#4ADE80" : c.test === "failed" ? "#FF5C7A" : c.test === "running" ? "#FFCC66" : "rgba(255,255,255,0.3)" }}>
                          {c.test === "passed" ? "✓ tests" : c.test === "failed" ? "✗ tests" : c.test === "running" ? "⟳ tests" : "—"}
                        </span>
                      </div>
                      <span>{c.time}</span>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}
