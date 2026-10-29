import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { KeyIcon, TrashIcon, RefreshIcon, TickIcon } from "../icons";
import { Button } from "./ui/button";

export type AuthFileSelection = { name: string; credentials: string } | { name: string; path: string };

interface Props {
  value: AuthFileSelection | null;
  fileLabel?: string;
  disabled: boolean;
  onChange: (file: AuthFileSelection | null) => void;
  onReadingChange: (reading: boolean) => void;
}

// Serialize mount/cleanup notifications, including StrictMode and rapid tab changes.
let panelUpdates: Promise<unknown> = Promise.resolve();
function setPanelFileImport(active: boolean): Promise<unknown> {
  panelUpdates = panelUpdates.catch(() => {}).then(() => invoke("set_panel_file_import", { active }));
  return panelUpdates;
}

/** Select a local JSON file, including Finder drops intercepted by Tauri's webview. */
export function AuthFileInput({ value, disabled, onChange, onReadingChange, fileLabel = "auth.json" }: Props) {
  const input = useRef<HTMLInputElement>(null);
  const zone = useRef<HTMLDivElement>(null);
  const reader = useRef<FileReader | null>(null);
  const reading = useRef(false);
  const latest = useRef({ disabled, onChange, onReadingChange, fileLabel });
  latest.current = { disabled, onChange, onReadingChange, fileLabel };
  const [dragging, setDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!isTauri()) return;
    let mounted = true;
    void setPanelFileImport(true).catch(() => {
      if (mounted) setError("Couldn't keep the import panel open. Reopen Import file to try again.");
    });
    return () => { mounted = false; void setPanelFileImport(false).catch(() => {}); };
  }, []);

  useEffect(() => () => {
    const pending = reader.current;
    if (pending) {
      pending.onload = pending.onerror = pending.onloadend = null;
      pending.abort();
    }
  }, []);
  useEffect(() => {
    if (!isTauri()) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    // Tauri reports physical pixels; DOM bounds use CSS pixels on Retina displays.
    void getCurrentWebview().onDragDropEvent(({ payload }) => {
      if (!active) return;
      if (payload.type === "leave" || latest.current.disabled || reading.current) {
        setDragging(false);
        return;
      }
      const bounds = zone.current?.getBoundingClientRect();
      const x = payload.position.x / window.devicePixelRatio;
      const y = payload.position.y / window.devicePixelRatio;
      const inside = !!bounds && x >= bounds.left && x <= bounds.right && y >= bounds.top && y <= bounds.bottom;
      setDragging(inside && payload.type !== "drop");
      if (payload.type !== "drop" || !inside) return;
      latest.current.onChange(null);
      if (payload.paths.length !== 1) {
        setError(`Choose one ${latest.current.fileLabel} file at a time.`);
        return;
      }
      const path = payload.paths[0];
      const name = path.split(/[\\/]/).pop() ?? "";
      if (!/\.json$/i.test(name)) {
        setError("Choose a JSON file containing your account login.");
        return;
      }
      setError(null);
      latest.current.onChange({ name, path });
    }).then((stop) => { if (active) unlisten = stop; else stop(); })
      .catch(() => { if (active) setError("Drag and drop is unavailable. Use Choose file instead."); });
    return () => { active = false; unlisten?.(); };
  }, []);

  const readFiles = (files: FileList | File[]) => {
    if (disabled || reading.current) return;
    setDragging(false);
    setError(null);
    onChange(null);
    if (files.length !== 1) {
      setError(`Choose one ${fileLabel} file at a time.`);
      return;
    }
    const file = files[0];
    if (!/\.json$/i.test(file.name)) {
      setError("Choose a JSON file containing your account login.");
      return;
    }
    if (file.size > 256 * 1024) {
      setError(`Use a ${fileLabel} smaller than 256 KB.`);
      return;
    }
    const next = new FileReader();
    reader.current = next;
    reading.current = true;
    setLoading(true);
    onReadingChange(true);
    next.onload = () => {
      const credentials = String(next.result ?? "");
      try { JSON.parse(credentials); }
      catch { setError(`This is not valid ${latest.current.fileLabel} JSON.`); return; }
      latest.current.onChange({ name: file.name, credentials });
    };
    next.onerror = () => setError("Couldn't read this file. Choose it again.");
    next.onloadend = () => {
      reading.current = false;
      reader.current = null;
      setLoading(false);
      latest.current.onReadingChange(false);
    };
    next.readAsText(file);
  };

  return <div className="auth-file-input">
    <input ref={input} type="file" accept=".json,application/json" hidden aria-label={`Choose ${fileLabel} file`} disabled={disabled || loading}
      onChange={(event) => { if (event.target.files?.length) readFiles(event.target.files); event.target.value = ""; }} />
    <div ref={zone} role="group" aria-label="Auth file upload" aria-disabled={disabled || loading} aria-busy={loading}
      className={`auth-file-drop${dragging ? " is-dragging" : ""}${value ? " is-filled" : ""}${error ? " has-error" : ""}`}
      onDragEnter={(event) => { event.preventDefault(); if (!disabled && !reading.current && event.dataTransfer.types.includes("Files")) setDragging(true); }}
      onDragOver={(event) => { event.preventDefault(); event.dataTransfer.dropEffect = disabled || reading.current ? "none" : "copy"; if (!disabled && !reading.current && event.dataTransfer.types.includes("Files")) setDragging(true); }}
      onDragLeave={(event) => { if (!(event.relatedTarget instanceof Node) || !event.currentTarget.contains(event.relatedTarget)) setDragging(false); }}
      onDrop={(event) => { event.preventDefault(); if (isTauri()) return; readFiles(event.dataTransfer.files); }}>
      <span className="auth-file-symbol" aria-hidden="true">{value && !dragging ? <TickIcon size={24} /> : <KeyIcon size={24} />}</span>
      <div className="auth-file-caption" role="status">
        <strong>{loading ? "Reading file…" : dragging ? "Drop to select this file" : value ? value.name : `Drop ${fileLabel} here`}</strong>
        <span>{dragging ? "Release to select your login" : value ? "Ready to import" : "or choose a JSON file from your Mac"}</span>
      </div>
      <div className="auth-file-actions">
        <Button size={value ? "icon-sm" : "sm"} variant={value ? "ghost" : "secondary"} aria-label={value ? "Choose another file" : "Choose file"} title={value ? "Choose another file" : "Choose file"} disabled={disabled || loading} onClick={() => input.current?.click()}>{value ? <RefreshIcon size={17} /> : "Choose file"}</Button>
        {value ? <Button size="icon-sm" variant="destructive" aria-label="Remove file" title="Remove file" disabled={disabled || loading} onClick={() => { onChange(null); setError(null); }}><TrashIcon size={17} /></Button> : null}
      </div>
    </div>
    {error ? <p className="workspace-error" role="alert">{error}</p> : null}
  </div>;
}
