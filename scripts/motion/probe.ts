/** Read-only frame/interaction probe, loaded only by the isolated motion fixture. */
export function installMotionProbe(context: { native: boolean; nativeVersion: unknown; production: boolean; fixture: boolean; scenario: () => string }) {
  const frames: number[] = [];
  const actions: Array<{ at: number; type: string; label: string; scenario: string }> = [];
  const errors: string[] = [];
  const layouts: object[] = [];
  const start = performance.now();
  const raf = (time: number) => { frames.push(time); requestAnimationFrame(raf); };
  requestAnimationFrame(raf);
  window.addEventListener("keydown", event => {
    if (event.ctrlKey && event.altKey && event.code === "Digit0") {
      frames.length = 0; actions.length = 0; errors.length = 0; layouts.length = 0;
    }
  });
  let timer = 0;
  const report = () => {
    const intervals = frames.slice(1).map((at, index) => ({ at, ms: at - frames[index] }));
    const active = intervals.filter(frame => actions.some(action => frame.at >= action.at && frame.at <= action.at + 700));
    const sorted = active.map(frame => frame.ms).sort((a, b) => a - b);
    const total = sorted.reduce((a, b) => a + b, 0);
    const payload = {
      ...context, scenario: context.scenario(), userAgent: navigator.userAgent,
      nativeOrigin: location.origin, viewport: [innerWidth, innerHeight], devicePixelRatio,
      elapsedMs: performance.now() - start, timeOrigin: performance.timeOrigin,
      frames, actions, errors, layouts,
      active: { frames: sorted.length, fps: sorted.length / total * 1000, p95Ms: sorted[Math.floor(sorted.length * .95)], maxMs: sorted.at(-1), over25ms: sorted.filter(ms => ms > 25).length, over34ms: sorted.filter(ms => ms > 34).length },
    };
    void fetch("http://127.0.0.1:55949", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(payload) }).catch(() => {});
  };
  const log = (event: Event) => {
    if (!event.isTrusted) return;
    const target = event.target instanceof Element ? event.target.closest("button, input, textarea, summary, [role=tab], [role=option]") : null;
    const label = target?.getAttribute("aria-label") ?? target?.getAttribute("role") ?? (target?.matches("input,textarea") ? "text input" : target?.textContent?.trim().slice(0, 90)) ?? event.type;
    actions.push({ at: performance.now(), type: event.type, label, scenario: context.scenario() });
    window.clearTimeout(timer);
    timer = window.setTimeout(report, 1000);
  };
  for (const type of ["pointerdown", "click", "input", "keydown", "wheel"]) window.addEventListener(type, log, true);
  window.addEventListener("error", event => { errors.push(event.message); report(); });
  window.addEventListener("unhandledrejection", event => { errors.push(String(event.reason)); report(); });
  new MutationObserver(() => {
    const dialog = document.querySelector("[role=dialog]");
    if (dialog) {
      const bounds = dialog.getBoundingClientRect();
      if (bounds.left < -1 || bounds.right > innerWidth + 1 || bounds.top < -1 || bounds.bottom > innerHeight + 1) layouts.push({ at: performance.now(), defect: "dialog outside viewport", bounds: bounds.toJSON() });
    }
  }).observe(document.body, { subtree: true, childList: true });
  window.setInterval(report, 5000);
  window.setTimeout(report, 2000);
}
