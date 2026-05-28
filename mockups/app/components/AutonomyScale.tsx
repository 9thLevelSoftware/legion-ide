import { useState, useRef, useEffect } from "react";
import { Keyboard, Sparkles, Users, Layers, Network } from "lucide-react";
import { motion, AnimatePresence } from "motion/react";

const LEVELS = [
  {
    n: 1,
    label: "Manual",
    icon: Keyboard,
    microcopy: "You write. AI stays quiet.",
    color: "#B6B7C3",
    bg: "rgba(255,255,255,0.06)",
    border: "rgba(255,255,255,0.16)",
    glow: "rgba(182, 183, 195, 0)",
  },
  {
    n: 2,
    label: "Assisted",
    icon: Sparkles,
    microcopy: "AI assists inline.",
    color: "#9EE9FF",
    bg: "rgba(57,215,255,0.10)",
    border: "rgba(57,215,255,0.45)",
    glow: "rgba(57,215,255, 0.2)",
  },
  {
    n: 3,
    label: "Co-Pilot",
    icon: Users,
    microcopy: "AI pairs with you.",
    color: "#A8C3FF",
    bg: "rgba(75,140,255,0.10)",
    border: "rgba(75,140,255,0.45)",
    glow: "rgba(75,140,255, 0.4)",
  },
  {
    n: 4,
    label: "Delegated",
    icon: Layers,
    microcopy: "Delegate scoped tasks.",
    color: "#C8B5FF",
    bg: "rgba(139,92,255,0.10)",
    border: "rgba(139,92,255,0.45)",
    glow: "rgba(139,92,255, 0.5)",
  },
  {
    n: 5,
    label: "Fleet",
    icon: Network,
    microcopy: "Fleet executes directives.",
    color: "#D9B8FF",
    bg: "rgba(177,108,255,0.12)",
    border: "rgba(177,108,255,0.5)",
    glow: "rgba(177,108,255, 0.7)",
  },
];

export function AutonomyScale({
  level,
  onLevel,
}: {
  level: number;
  onLevel: (n: number) => void;
}) {
  const [hoveredLevel, setHoveredLevel] = useState<number | null>(null);
  const [pendingLevel, setPendingLevel] = useState<number | null>(null);

  const handleLevelClick = (n: number) => {
    if (n === level) return;
    if (n >= 4 && n > level) {
      setPendingLevel(n);
    } else {
      onLevel(n);
    }
  };

  const confirmPending = () => {
    if (pendingLevel !== null) {
      onLevel(pendingLevel);
      setPendingLevel(null);
    }
  };

  const cancelPending = () => {
    setPendingLevel(null);
  };

  return (
    <div className="relative flex flex-col items-center">
      <div
        className="relative flex items-center rounded-full p-[3px] shadow-inner"
        style={{
          background: "#08080C",
          border: "1px solid rgba(255,255,255,0.08)",
          boxShadow: "inset 0 1px 3px rgba(0,0,0,0.5)",
        }}
        onMouseLeave={() => setHoveredLevel(null)}
      >
        {LEVELS.map((l) => {
          const isActive = l.n === level;
          const isHovered = l.n === hoveredLevel;
          const isPending = l.n === pendingLevel;

          const Icon = l.icon;

          return (
            <div
              key={l.n}
              className="relative group"
              onMouseEnter={() => setHoveredLevel(l.n)}
            >
              <button
                onClick={() => handleLevelClick(l.n)}
                className={`relative z-10 flex items-center gap-1.5 px-3 h-[26px] rounded-full text-[11px] font-medium transition-colors ${
                  isActive ? "text-white" : "text-white/45 hover:text-white/80"
                }`}
              >
                {isActive && (
                  <motion.div
                    layoutId="active-pill"
                    className="absolute inset-0 rounded-full z-0"
                    style={{
                      background: l.bg,
                      border: `1px solid ${l.border}`,
                      boxShadow: l.n === 5 ? `0 0 12px ${l.glow}` : `0 0 8px ${l.glow}`,
                    }}
                    transition={{ type: "spring", stiffness: 400, damping: 30 }}
                  />
                )}
                <Icon
                  className="relative z-10 w-3 h-3 transition-colors"
                  style={{ color: isActive ? l.color : "inherit" }}
                />
                <span className="relative z-10">{l.label}</span>
                <span
                  className="relative z-10 font-mono text-[9px] opacity-70"
                  style={{ color: isActive ? l.color : "inherit" }}
                >
                  L{l.n}
                </span>
              </button>
            </div>
          );
        })}
      </div>

      {/* Tooltip */}
      <AnimatePresence>
        {hoveredLevel !== null && !pendingLevel && (
          <motion.div
            initial={{ opacity: 0, y: 4, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 2, scale: 0.95 }}
            transition={{ duration: 0.15 }}
            className="absolute top-full mt-2 px-2.5 py-1.5 rounded-md text-[11px] whitespace-nowrap pointer-events-none z-50 shadow-lg"
            style={{
              background: "#181824",
              border: "1px solid rgba(255,255,255,0.1)",
              color: "rgba(255,255,255,0.85)",
            }}
          >
            {LEVELS.find((l) => l.n === hoveredLevel)?.microcopy}
          </motion.div>
        )}
      </AnimatePresence>

      {/* Confirmation Popover */}
      <AnimatePresence>
        {pendingLevel !== null && (
          <motion.div
            initial={{ opacity: 0, y: 8, scale: 0.95 }}
            animate={{ opacity: 1, y: 0, scale: 1 }}
            exit={{ opacity: 0, y: 4, scale: 0.95 }}
            transition={{ type: "spring", stiffness: 500, damping: 30 }}
            className="absolute top-full mt-2 p-3 rounded-lg shadow-xl z-50 flex flex-col gap-2 min-w-[200px]"
            style={{
              background: "#1A1A24",
              border: "1px solid rgba(255,255,255,0.15)",
              boxShadow: "0 10px 30px rgba(0,0,0,0.5)",
            }}
          >
            <div className="text-[12px] font-medium text-white">
              {pendingLevel === 4
                ? "Delegate scoped work?"
                : "Activate Autonomous Fleet?"}
            </div>
            <div className="text-[11px] text-white/60 leading-relaxed">
              {pendingLevel === 4
                ? "AI agents will execute tasks and request your approval."
                : "The fleet will autonomously plan and execute across your codebase."}
            </div>
            <div className="flex items-center gap-2 mt-1">
              <button
                onClick={cancelPending}
                className="flex-1 px-2 py-1.5 rounded-md text-[11px] text-white/70 hover:bg-white/10 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={confirmPending}
                className="flex-1 px-2 py-1.5 rounded-md text-[11px] font-medium text-white transition-colors"
                style={{
                  background:
                    pendingLevel === 4
                      ? "rgba(139,92,255,0.5)"
                      : "rgba(177,108,255,0.5)",
                  border: `1px solid ${
                    pendingLevel === 4
                      ? "rgba(139,92,255,0.8)"
                      : "rgba(177,108,255,0.8)"
                  }`,
                }}
              >
                Confirm
              </button>
            </div>
          </motion.div>
        )}
      </AnimatePresence>
    </div>
  );
}
