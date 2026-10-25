import React, { useEffect, useRef, useState } from 'react';
import { AlertTriangle, Check, Loader2 } from 'lucide-react';
import { appBootstrap, listenToBootstrapProgress } from '../lib/ipc';
import { BootstrapProgress } from '../types';

interface BootStep {
  stage: string;
  label: string;
  detail?: string | null;
  percent?: number | null;
  finished: boolean;
}

/**
 * Where each stage sits on the overall bar. Within a stage the reported
 * percentage interpolates, so the bar keeps moving even when a stage has no
 * bytes to count (an archive download reports `null`).
 */
const STAGE_WEIGHT: Record<string, [number, number]> = {
  files: [0, 12],
  steamcmd: [12, 46],
  library: [46, 100],
  done: [100, 100],
};

export const BootScreen: React.FC<{ onDone: () => void }> = ({ onDone }) => {
  const [steps, setSteps] = useState<BootStep[]>([]);
  const [error, setError] = useState<string | null>(null);

  // Held in a ref so the startup effect never restarts when the parent re-renders.
  const doneRef = useRef(onDone);
  doneRef.current = onDone;

  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    const run = async () => {
      unlisten = await listenToBootstrapProgress((event: { payload: BootstrapProgress }) => {
        const payload = event.payload;

        setSteps((prev) => {
          const entry: BootStep = {
            stage: payload.stage,
            label: payload.label,
            detail: payload.detail,
            percent: payload.percent,
            finished: payload.finished,
          };

          const index = prev.findIndex((step) => step.stage === payload.stage);
          if (index === -1) {
            return [...prev, entry];
          }

          const next = [...prev];
          next[index] = entry;
          return next;
        });
      });

      if (cancelled) {
        unlisten?.();
        return;
      }

      try {
        await appBootstrap(false);
        // Let the "done" row render before the launcher takes over.
        if (!cancelled) {
          setTimeout(() => doneRef.current(), 550);
        }
      } catch (e) {
        if (!cancelled) {
          setError(String(e));
        }
      }
    };

    run();

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  const overall = steps.reduce((highest, step) => {
    const [start, end] = STAGE_WEIGHT[step.stage] ?? [100, 100];
    const within = typeof step.percent === 'number' ? step.percent : step.finished ? 100 : 35;
    const value = start + ((end - start) * within) / 100;
    return Math.max(highest, Math.min(100, value));
  }, error ? 100 : 4);

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center overflow-hidden bg-[#050507] text-white select-none">
      {/* Same orange glow language as the shell, so startup does not look like another app */}
      <div className="pointer-events-none absolute -bottom-40 -right-32 h-[520px] w-[520px] rounded-full bg-orange-600/20 blur-[140px]" />
      <div className="pointer-events-none absolute -top-40 -left-32 h-[420px] w-[420px] rounded-full bg-orange-500/10 blur-[120px]" />

      <div className="relative w-[min(520px,90vw)]">
        <div className="text-center">
          <div className="text-[28px] font-black tracking-[0.28em] text-white">HYPERION</div>
          <div className="mt-1 text-[10px] font-semibold uppercase tracking-[0.42em] text-orange-400/90">
            Launcher
          </div>
        </div>

        <div className="mt-8 space-y-3">
          {steps.length === 0 && (
            <div className="flex items-center gap-3 rounded-xl border border-zinc-800/80 bg-zinc-900/50 px-4 py-3">
              <Loader2 className="h-4 w-4 animate-spin text-orange-400" />
              <span className="text-[13px] font-semibold text-white">Waking up the backend…</span>
            </div>
          )}

          {steps.map((step) => (
            <div
              key={step.stage}
              className="flex items-start gap-3 rounded-xl border border-zinc-800/80 bg-zinc-900/50 px-4 py-3"
            >
              <span className="mt-0.5">
                {step.finished ? (
                  <Check className="h-4 w-4 text-emerald-400" />
                ) : (
                  <Loader2 className="h-4 w-4 animate-spin text-orange-400" />
                )}
              </span>

              <div className="min-w-0 flex-1">
                <div className="text-[13px] font-semibold text-white">{step.label}</div>
                {step.detail && (
                  <div className="mt-0.5 text-[11px] leading-relaxed text-zinc-400">{step.detail}</div>
                )}
              </div>

              {typeof step.percent === 'number' && !step.finished && (
                <span className="font-mono text-[11px] text-zinc-500">{Math.round(step.percent)}%</span>
              )}
            </div>
          ))}
        </div>

        {error ? (
          <div className="mt-6 rounded-xl border border-red-500/30 bg-red-500/10 px-4 py-3">
            <div className="flex items-center gap-2 text-[12px] font-semibold text-red-300">
              <AlertTriangle className="h-4 w-4" />
              Startup hit a problem
            </div>
            <p className="mt-1 text-[11px] leading-relaxed text-red-200/80">{error}</p>
            <button
              type="button"
              onClick={() => doneRef.current()}
              className="mt-3 rounded-lg bg-white/10 px-3 py-1.5 text-[11px] font-bold text-white transition hover:bg-white/20"
            >
              Continue anyway
            </button>
          </div>
        ) : (
          <div className="mt-8">
            <div className="h-1 w-full overflow-hidden rounded-full bg-zinc-800">
              <div
                className="h-full rounded-full bg-gradient-to-r from-orange-500 to-amber-400 transition-[width] duration-500"
                style={{ width: `${overall}%` }}
              />
            </div>
            <div className="mt-2 flex justify-between font-mono text-[10px] uppercase tracking-widest text-zinc-600">
              <span>Starting up</span>
              <span>{Math.round(overall)}%</span>
            </div>
          </div>
        )}
      </div>
    </div>
  );
};
