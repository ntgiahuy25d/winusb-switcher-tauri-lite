import { useCallback, useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { useProbeStore } from "./store/probeStore";
import Dashboard from "./pages/Dashboard";
import { COMMANDS } from "@shared/types";

const BODY_PADDING = 24; // 12px top + 12px bottom (matches body padding in styles.css)

// Resize only the window HEIGHT to fit content; preserves whatever width the user has set.
async function resizeHeightToContent() {
  await new Promise<void>((r) => setTimeout(r, 80)); // wait one paint for DOM to settle
  const card =
    document.querySelector<HTMLElement>(".app-card") ??
    document.querySelector<HTMLElement>(".message-card") ??
    document.querySelector<HTMLElement>(".bootstrap-lite-card");
  if (!card) return;
  const targetHeight = card.scrollHeight + BODY_PADDING;
  try {
    const win = getCurrentWindow();
    const [physicalSize, scale] = await Promise.all([win.outerSize(), win.scaleFactor()]);
    const currentLogicalWidth = Math.round(physicalSize.width / scale);
    await win.setSize(new LogicalSize(currentLogicalWidth, targetHeight));
  } catch (e) {
    console.error("[App] window resize failed:", e);
  }
}

export default function App() {
  const { isInstalled, checkInstallation } = useProbeStore();
  const [bootstrap, setBootstrap] = useState<"pending" | "ok" | "error">("pending");
  const [bootstrapError, setBootstrapError] = useState<string>("");

  const runBootstrap = useCallback(async () => {
    setBootstrap("pending");
    setBootstrapError("");
    try {
      await invoke<string>(COMMANDS.PREPARE_BUNDLED_JLINK);
      setBootstrap("ok");
    } catch (err) {
      setBootstrap("error");
      setBootstrapError(err instanceof Error ? err.message : String(err));
    }
  }, []);

  useEffect(() => {
    runBootstrap();
  }, [runBootstrap]);

  useEffect(() => {
    if (bootstrap !== "ok") return;
    checkInstallation().catch((err) => {
      console.error("[App] checkInstallation failed:", err);
    });
  }, [bootstrap, checkInstallation]);

  useEffect(() => {
    resizeHeightToContent();
  }, [bootstrap, isInstalled]);

  if (bootstrap === "pending") {
    return (
      <div className="flex items-center justify-center min-h-screen bg-gradient-to-b from-slate-50 to-white p-8">
        <div className="bootstrap-lite-card w-full max-w-[420px] rounded-xl border border-slate-200/80 bg-white/90 px-8 py-10 shadow-[0_8px_30px_rgb(0,0,0,0.06)] backdrop-blur-sm">
          <div className="mx-auto mb-6 h-px w-12 rounded-full bg-gradient-to-r from-transparent via-slate-300 to-transparent" aria-hidden />
          <h1 className="text-center text-base font-semibold tracking-tight text-slate-800">
            Initializing WinUSB Switcher Lite
          </h1>
          <p className="mt-3 text-center text-[13px] leading-relaxed text-slate-600">
            A one-time setup is preparing the embedded J-Link components. This usually completes in under a minute.
          </p>
          <p className="mt-5 text-center text-xs text-slate-500">Please keep this window open.</p>
          <div className="mt-8 h-1 overflow-hidden rounded-full bg-slate-100" role="progressbar" aria-label="Setup in progress">
            <div className="bootstrap-lite-progress h-full w-2/5 rounded-full bg-slate-500/85" />
          </div>
          <style>{`
            .bootstrap-lite-progress {
              animation: bootstrapLiteShimmer 1.35s ease-in-out infinite;
            }
            @keyframes bootstrapLiteShimmer {
              0% { transform: translateX(-120%); }
              100% { transform: translateX(320%); }
            }
          `}</style>
        </div>
      </div>
    );
  }

  if (bootstrap === "error") {
    return (
      <div className="flex items-center justify-center min-h-screen bg-gradient-to-b from-slate-50 to-white p-8">
        <div className="bootstrap-lite-card w-full max-w-[420px] rounded-xl border border-red-200/90 bg-white px-8 py-10 shadow-[0_8px_30px_rgb(0,0,0,0.06)]">
          <h1 className="text-center text-[15px] font-semibold tracking-tight text-red-900">
            Setup could not finish
          </h1>
          <p className="mt-3 text-center text-[13px] leading-relaxed text-red-800/90 break-words">
            {bootstrapError}
          </p>
          <div className="mt-8 flex justify-center">
            <button type="button" className="btn btn-primary" onClick={() => void runBootstrap()}>
              Try again
            </button>
          </div>
        </div>
      </div>
    );
  }

  if (isInstalled === null) {
    return (
      <div className="flex items-center justify-center h-screen bg-white">
        <div className="text-gray-400 text-sm">Checking J-Link installation...</div>
      </div>
    );
  }

  return <Dashboard />;
}
