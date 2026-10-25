import React from 'react';
import { KeyRound, Lock, ShieldCheck } from 'lucide-react';

/**
 * Full-window sign-in gate.
 *
 * Sits above the shell (z-40) and below the login modal (z-50), so pressing the
 * primary action opens the real Epic flow on top of it rather than duplicating
 * OAuth handling here.
 */
export const LoginGate: React.FC<{
  onSignIn: () => void;
  onSkip: () => void;
}> = ({ onSignIn, onSkip }) => {
  return (
    <div className="fixed inset-0 z-40 flex items-center justify-center overflow-hidden bg-[#050507]/95 backdrop-blur-xl">
      <div className="pointer-events-none absolute -bottom-40 right-0 h-[520px] w-[520px] rounded-full bg-orange-600/20 blur-[140px]" />
      <div className="pointer-events-none absolute -top-40 left-0 h-[420px] w-[420px] rounded-full bg-orange-500/10 blur-[120px]" />

      <div className="relative w-[min(440px,90vw)] rounded-3xl border border-zinc-800/80 bg-zinc-950/80 p-8 shadow-2xl shadow-black/60">
        <div className="text-center">
          <div className="text-[22px] font-black tracking-[0.24em] text-white">HYPERION</div>
          <div className="mt-1 text-[10px] font-semibold uppercase tracking-[0.4em] text-orange-400/90">
            Launcher
          </div>
        </div>

        <h2 className="mt-7 text-center text-[17px] font-bold text-white">Sign in to continue</h2>
        <p className="mt-2 text-center text-[12px] leading-relaxed text-zinc-400">
          Your Epic library, install sizes and cloud saves are tied to your account.
        </p>

        <button
          type="button"
          onClick={onSignIn}
          className="mt-6 flex w-full items-center justify-center gap-2 rounded-xl bg-gradient-to-r from-orange-600 to-amber-500 px-4 py-3 text-[13px] font-extrabold text-white shadow-lg shadow-orange-600/25 transition hover:brightness-110"
        >
          <KeyRound className="h-4 w-4" />
          Sign in with Epic
        </button>

        <div className="mt-3 flex items-center justify-center gap-1.5 text-[10px] text-zinc-500">
          <Lock className="h-3 w-3" />
          Epic&apos;s own login page handles your password — it never touches this app
        </div>

        <div className="mt-6 border-t border-zinc-900 pt-4">
          <button
            type="button"
            onClick={onSkip}
            className="w-full rounded-lg px-3 py-2 text-[11px] font-semibold text-zinc-400 transition hover:bg-white/5 hover:text-zinc-200"
          >
            Continue offline — installed games only
          </button>
          <div className="mt-1 flex items-center justify-center gap-1.5 text-[10px] text-zinc-600">
            <ShieldCheck className="h-3 w-3" />
            Downloads and cloud saves stay unavailable until you sign in
          </div>
        </div>
      </div>
    </div>
  );
};
