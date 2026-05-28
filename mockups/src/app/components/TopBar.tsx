import { AutonomyScale } from "./AutonomyScale";
import {
  Search,
  GitBranch,
  Command,
  Bell,
  Settings2,
  Play,
  Cpu,
  MemoryStick,
  Coins,
  Users,
  ChevronDown,
} from "lucide-react";

function ResourceChip({ icon: Icon, value, sub, color = "#B6B7C3" }: any) {
  return (
    <div className="flex items-center gap-1.5 h-6 px-1.5 rounded-md text-[10.5px] text-white/55 hover:bg-white/[0.04]">
      <Icon className="w-3 h-3" style={{ color }} />
      <span className="font-mono text-white/75">{value}</span>
      {sub && <span className="text-white/35">{sub}</span>}
    </div>
  );
}

const LEVEL_STATUS: Record<number, { label: string; engine: string; engineColor: string; pulse: boolean }> = {
  1: { label: "Manual Mode", engine: "Legion Engine Idle", engineColor: "#6B6E7D", pulse: false },
  2: { label: "Assisted Coding Active", engine: "Context indexed", engineColor: "#39D7FF", pulse: true },
  3: { label: "Pair Programming Active", engine: "Primary Co-Pilot: GPT-5.5", engineColor: "#4B8CFF", pulse: true },
  4: { label: "Delegated Tasks Active", engine: "4 agents · 3 approvals pending", engineColor: "#8B5CFF", pulse: true },
  5: { label: "Autonomous Fleet Active", engine: "Legion Engine Online", engineColor: "#B16CFF", pulse: true },
};

export function TopBar({ level, onLevel }: { level: number; onLevel: (n: number) => void }) {
  const status = LEVEL_STATUS[level];
  return (
    <div
      className="h-[52px] shrink-0 flex items-center justify-between px-3 border-b select-none relative z-50"
      style={{ background: "#111118", borderColor: "rgba(255,255,255,0.05)" }}
    >
      {/* Left */}
      <div className="flex items-center gap-3 min-w-0 flex-1">
        <div className="flex items-center gap-1.5 pl-1">
          <span className="w-3 h-3 rounded-full bg-[#FF5F57]" />
          <span className="w-3 h-3 rounded-full bg-[#FEBC2E]" />
          <span className="w-3 h-3 rounded-full bg-[#28C840]" />
        </div>
        {/* Legion mark */}
        <div className="flex items-center gap-2 pl-1">
          <svg width="18" height="18" viewBox="0 0 18 18" className="text-white/85">
            <path d="M3 2 L9 2 L15 8 L9 14 L9 8 L3 8 Z" fill="currentColor" opacity="0.9" />
            <rect x="3" y="11" width="6" height="2" fill="currentColor" opacity="0.55" />
          </svg>
          <span className="text-[12.5px] text-white font-medium tracking-tight">Legion</span>
        </div>
        <span className="w-px h-4 bg-white/[0.08]" />
        <div className="flex items-center gap-2 text-[12px] text-white/65">
          <span className="text-white/90 font-medium">nebula-commerce</span>
          <ChevronDown className="w-3 h-3 text-white/35" />
          <span className="inline-flex items-center gap-1 px-1.5 h-5 rounded bg-white/[0.04] border border-white/[0.07] text-[11px] text-white/60">
            <GitBranch className="w-3 h-3" /> feature/stripe-subscriptions
          </span>
          <span className="inline-flex items-center gap-1.5 px-1.5 h-5 rounded text-[10.5px] text-white/55">
            <span className="relative flex">
              <span className="w-1.5 h-1.5 rounded-full" style={{ background: status.engineColor }} />
              {status.pulse && (
                <span
                  className="absolute inset-0 w-1.5 h-1.5 rounded-full animate-ping opacity-60"
                  style={{ background: status.engineColor }}
                />
              )}
            </span>
            {status.engine}
          </span>
          <span
            className="ml-1 inline-flex items-center px-1.5 h-5 rounded text-[10px] font-medium"
            style={{
              color: level === 1 ? "#B6B7C3" : "#F4F4F6",
              background: level === 1 ? "rgba(255,255,255,0.04)" : "rgba(255,255,255,0.06)",
              border: "1px solid rgba(255,255,255,0.08)",
            }}
          >
            {status.label}
          </span>
        </div>
      </div>

      {/* Center: Autonomy Scale */}
      <div className="absolute left-1/2 -translate-x-1/2 flex flex-col items-center z-50">
        <span className="text-[9.5px] uppercase tracking-[0.18em] text-white/30 mb-1 font-medium">
          Autonomy Level
        </span>
        <AutonomyScale level={level} onLevel={onLevel} />
      </div>

      {/* Right */}
      <div className="flex items-center gap-1.5 flex-1 justify-end">
        <button className="h-7 px-2.5 flex items-center gap-2 rounded-md bg-white/[0.03] border border-white/[0.07] text-[11px] text-white/55 hover:text-white/85 hover:bg-white/[0.06]">
          <Search className="w-3.5 h-3.5" />
          <span>Run command</span>
          <span className="ml-1 inline-flex items-center gap-0.5 text-[10px] text-white/35 font-mono">
            <Command className="w-3 h-3" />K
          </span>
        </button>

        {/* Build status */}
        <div className="h-7 px-2 flex items-center gap-1.5 rounded-md bg-white/[0.03] border border-white/[0.07] text-[10.5px]">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400" />
          <span className="text-white/70">Build passing</span>
          <span className="text-white/35 font-mono">412 tests</span>
        </div>

        {/* Resources */}
        <div className="h-7 flex items-center gap-0 rounded-md bg-white/[0.02] border border-white/[0.06] px-1">
          {level === 5 ? (
            <>
              <ResourceChip icon={Cpu} value="82%" color="#39D7FF" />
              <span className="w-px h-3 bg-white/[0.07]" />
              <ResourceChip icon={Coins} value="842k" sub="/ 1M" color="#FFCC66" />
              <span className="w-px h-3 bg-white/[0.07]" />
              <ResourceChip icon={Users} value="9" sub="agents" color="#B16CFF" />
              <span className="w-px h-3 bg-white/[0.07]" />
              <ResourceChip icon={Settings2} value="4" sub="workflows" color="#39D7FF" />
            </>
          ) : (
            <>
              <ResourceChip icon={Cpu} value="34%" color="#39D7FF" />
              <span className="w-px h-3 bg-white/[0.07]" />
              <ResourceChip icon={MemoryStick} value="2.1G" color="#4B8CFF" />
              <span className="w-px h-3 bg-white/[0.07]" />
              <ResourceChip icon={Coins} value="184k" sub="/ 1M" color="#FFCC66" />
              <span className="w-px h-3 bg-white/[0.07]" />
              <ResourceChip icon={Users} value="4" sub="agents" color="#8B5CFF" />
            </>
          )}
        </div>

        {/* Primary/Secondary Buttons */}
        {level === 5 ? (
          <>
            <button className="h-7 px-2.5 flex items-center gap-1.5 rounded-md bg-white/[0.03] border border-white/[0.07] text-[11px] text-white/70 hover:text-white/90 hover:bg-white/[0.06]">
              Review Decisions
            </button>
            <button
              className="h-7 pl-2 pr-2.5 flex items-center gap-1.5 rounded-md text-[11px] font-semibold text-white/90 bg-white/10 hover:bg-white/15"
              style={{
                border: "1px solid rgba(255,255,255,0.1)",
              }}
            >
              <span className="w-2 h-2 bg-[#FF5C7A] rounded-sm" />
              Pause Fleet
            </button>
          </>
        ) : (
          <button
            className="h-7 pl-2 pr-2.5 flex items-center gap-1.5 rounded-md text-[11px] font-semibold"
            style={{
              color: "#09090D",
              background: "linear-gradient(135deg, #39D7FF 0%, #8B5CFF 100%)",
            }}
          >
            <Play className="w-3 h-3 fill-current" />
            Run Directive
          </button>
        )}

        <button className="h-7 w-7 grid place-items-center rounded-md hover:bg-white/[0.05] text-white/45">
          <Bell className="w-3.5 h-3.5" />
        </button>
        <div className="ml-0.5 w-7 h-7 rounded-full bg-white/[0.06] border border-white/[0.1] grid place-items-center text-[10px] font-medium text-white/80">
          MK
        </div>
      </div>
    </div>
  );
}
