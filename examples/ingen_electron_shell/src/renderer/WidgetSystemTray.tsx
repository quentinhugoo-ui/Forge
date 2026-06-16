import { useEffect, useState } from "react";

/* Widget-mode system tray: when the real Windows taskbar is hidden, this
   mirrors the bottom-right indicator cluster (network, volume, battery, clock)
   in the floating widget. Phase 1 is renderer-backed (clock/date are exact,
   battery/online via web APIs); real volume level and Wi-Fi signal arrive from
   a native snapshot in a later phase. Clicking an indicator opens the matching
   Windows panel through an allow-listed shell URI. */

/* Real Win11 tray behaviour: clicking a system indicator opens the Quick
   Settings flyout (Win+A) and clicking the clock opens the calendar /
   notifications flyout (Win+N), rather than the full Settings app. */
type WidgetShellAction = "quick-settings" | "calendar";
type ForgeShellTrayApi = { triggerWidgetShellAction?: (action: WidgetShellAction) => Promise<boolean> };

type BatteryLike = {
  level: number;
  charging: boolean;
  addEventListener: (type: string, listener: () => void) => void;
  removeEventListener: (type: string, listener: () => void) => void;
};
type BatteryNavigator = Navigator & { getBattery?: () => Promise<BatteryLike> };

const TRAY_CLOCK_TICK_MS = 1000;

function triggerWidgetShellAction(action: WidgetShellAction): void {
  const api = (globalThis.window as unknown as { forgeShell?: ForgeShellTrayApi })?.forgeShell;
  void api?.triggerWidgetShellAction?.(action);
}

function formatTime(now: Date): string {
  return `${String(now.getHours()).padStart(2, "0")}:${String(now.getMinutes()).padStart(2, "0")}`;
}

function formatDate(now: Date): string {
  return `${String(now.getDate()).padStart(2, "0")}/${String(now.getMonth() + 1).padStart(2, "0")}/${now.getFullYear()}`;
}

function useClock(active: boolean): Date {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    if (!active) {
      return undefined;
    }
    setNow(new Date());
    const timer = window.setInterval(() => setNow(new Date()), TRAY_CLOCK_TICK_MS);
    return () => window.clearInterval(timer);
  }, [active]);
  return now;
}

function useOnline(active: boolean): boolean {
  const [online, setOnline] = useState(() => navigator.onLine);
  useEffect(() => {
    if (!active) {
      return undefined;
    }
    const update = () => setOnline(navigator.onLine);
    update();
    window.addEventListener("online", update);
    window.addEventListener("offline", update);
    return () => {
      window.removeEventListener("online", update);
      window.removeEventListener("offline", update);
    };
  }, [active]);
  return online;
}

function useBattery(active: boolean): { level: number; charging: boolean } | null {
  const [state, setState] = useState<{ level: number; charging: boolean } | null>(null);
  useEffect(() => {
    if (!active) {
      return undefined;
    }
    const getBattery = (navigator as BatteryNavigator).getBattery;
    if (!getBattery) {
      return undefined;
    }
    let battery: BatteryLike | null = null;
    let cancelled = false;
    const sync = () => {
      if (battery) {
        setState({ level: battery.level, charging: battery.charging });
      }
    };
    void getBattery.call(navigator).then((value) => {
      if (cancelled) {
        return;
      }
      battery = value;
      sync();
      value.addEventListener("levelchange", sync);
      value.addEventListener("chargingchange", sync);
    });
    return () => {
      cancelled = true;
      battery?.removeEventListener("levelchange", sync);
      battery?.removeEventListener("chargingchange", sync);
    };
  }, [active]);
  return state;
}

function WifiGlyph({ online }: { online: boolean }) {
  return (
    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
      <path
        d="M10 15.4a1.4 1.4 0 1 0 0-2.8 1.4 1.4 0 0 0 0 2.8Z"
        fill="currentColor"
        opacity={online ? 1 : 0.4}
      />
      <path
        d="M5.4 10.6a6.6 6.6 0 0 1 9.2 0M2.8 7.9a10.3 10.3 0 0 1 14.4 0"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
        opacity={online ? 1 : 0.4}
      />
      {!online ? (
        <path d="M3.5 3.5 16.5 16.5" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" />
      ) : null}
    </svg>
  );
}

function VolumeGlyph() {
  return (
    <svg viewBox="0 0 20 20" aria-hidden="true" focusable="false">
      <path d="M4 7.5h2.5L10.5 4v12L6.5 12.5H4Z" fill="currentColor" />
      <path
        d="M13 7a4 4 0 0 1 0 6M15 5a7 7 0 0 1 0 10"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.5"
        strokeLinecap="round"
      />
    </svg>
  );
}

function BatteryGlyph({ level, charging }: { level: number; charging: boolean }) {
  const fillWidth = Math.max(0.06, Math.min(1, level)) * 12.4;
  return (
    <svg viewBox="0 0 26 20" aria-hidden="true" focusable="false">
      <rect x="2.5" y="6" width="16" height="8" rx="1.6" fill="none" stroke="currentColor" strokeWidth="1.4" />
      <rect x="19" y="8.6" width="1.8" height="2.8" rx="0.9" fill="currentColor" />
      <rect x="4.3" y="7.8" width={fillWidth} height="4.4" rx="0.8" fill="currentColor" />
      {charging ? (
        <path d="M11.4 5.6 8.6 10.4h2.4l-0.6 4 3.4-5h-2.4Z" fill="currentColor" stroke="var(--forge-chat-bg, #111)" strokeWidth="0.5" />
      ) : null}
    </svg>
  );
}

export function WidgetSystemTray({ visible }: { visible: boolean }) {
  const now = useClock(visible);
  const online = useOnline(visible);
  const battery = useBattery(visible);
  if (!visible) {
    return null;
  }
  const batteryPercent = battery ? Math.round(battery.level * 100) : null;
  return (
    <aside className="widgetSystemTray" aria-label="System indicators">
      <button
        type="button"
        className="widgetSystemTray__icon"
        aria-label="Network"
        onClick={() => triggerWidgetShellAction("quick-settings")}
      >
        <WifiGlyph online={online} />
      </button>
      <button
        type="button"
        className="widgetSystemTray__icon"
        aria-label="Volume"
        onClick={() => triggerWidgetShellAction("quick-settings")}
      >
        <VolumeGlyph />
      </button>
      <button
        type="button"
        className="widgetSystemTray__battery"
        aria-label={batteryPercent !== null ? `Battery ${batteryPercent}%` : "Battery"}
        onClick={() => triggerWidgetShellAction("quick-settings")}
      >
        {battery ? <BatteryGlyph level={battery.level} charging={battery.charging} /> : <BatteryGlyph level={1} charging={false} />}
        {batteryPercent !== null ? <span className="widgetSystemTray__batteryPct">{batteryPercent}%</span> : null}
      </button>
      <button
        type="button"
        className="widgetSystemTray__clock"
        aria-label={`${formatTime(now)} ${formatDate(now)}`}
        onClick={() => triggerWidgetShellAction("calendar")}
      >
        <span className="widgetSystemTray__time">{formatTime(now)}</span>
        <span className="widgetSystemTray__date">{formatDate(now)}</span>
      </button>
    </aside>
  );
}
