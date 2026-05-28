import { useState } from "react";
import { TopBar } from "./components/TopBar";
import { LeftSidebar } from "./components/LeftSidebar";
import { CodeCanvas } from "./components/CodeCanvas";
import { RightInspector } from "./components/RightInspector";
import { BottomConsole } from "./components/BottomConsole";

export default function App() {
  const [level, setLevel] = useState(1);
  const manual = level === 1;

  return (
    <div
      className="size-full flex flex-col text-white antialiased overflow-hidden"
      style={{
        fontFamily:
          "'Inter', -apple-system, BlinkMacSystemFont, 'SF Pro Display', system-ui, sans-serif",
        background: "#0D0D12",
      }}
    >
      <style>{`
        .tk-key { color: #C8B5FF; }
        .tk-typ { color: #9EE9FF; }
        .tk-fn  { color: #FFCC66; }
        .tk-str { color: #86EFAC; }
        .tk-num { color: #FF5C7A; }
        .tk-cmt { color: rgba(255,255,255,0.32); font-style: italic; }
        .tk-id  { color: #F4F4F6; }
        code, pre, .font-mono { font-family: "JetBrains Mono", "Berkeley Mono", "Geist Mono", "SF Mono", Consolas, monospace; }
        ::-webkit-scrollbar { width: 8px; height: 8px; }
        ::-webkit-scrollbar-thumb { background: rgba(255,255,255,0.06); border-radius: 8px; }
        ::-webkit-scrollbar-thumb:hover { background: rgba(255,255,255,0.12); }
        ::-webkit-scrollbar-track { background: transparent; }
      `}</style>

      <TopBar level={level} onLevel={setLevel} />

      <div className="flex-1 min-h-0 flex">
        <LeftSidebar level={level} />
        <div className="flex-1 min-w-0 flex flex-col">
          <div className="flex-1 min-h-0 flex">
            <CodeCanvas level={level} />
            <RightInspector level={level} />
          </div>
          <BottomConsole level={level} />
        </div>
      </div>

      {/* Status bar */}
      <div
        className="h-6 shrink-0 flex items-center justify-between px-3 border-t text-[10.5px] text-white/45"
        style={{ background: "#0B0B10", borderColor: "rgba(255,255,255,0.05)" }}
      >
        <div className="flex items-center gap-3">
          <span className="flex items-center gap-1.5">
            <span className="w-1.5 h-1.5 rounded-full" style={{ background: "#4ADE80" }} /> connected · {manual ? "local tools" : "model runtime"}
          </span>
          <span className="font-mono">feature/stripe-subscriptions</span>
          <span className="font-mono">↑2 ↓0</span>
          <span>TypeScript 5.6</span>
          {manual && <span>AI disabled · no model calls</span>}
        </div>
        <div className="flex items-center gap-3 font-mono">
          <span>Ln 15, Col 22</span>
          <span>UTF-8</span>
          <span>LF</span>
          <span style={{ color: manual ? "#A8C3FF" : "#C8B5FF" }}>
            {manual ? "Manual · AI Disabled" : level === 2 ? "Delegates" : "Legion Workflows"}
          </span>
        </div>
      </div>
    </div>
  );
}
