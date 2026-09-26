import { useEffect, useRef, useState } from "react";
import type { ProfileRef } from "../types";
import { AddIcon, TickIcon } from "../icons";

/** Header chip: shows the active profile and switches between profiles. */
export function ProfileMenu({
  profiles,
  active,
  busy,
  onSwitch,
  onManage,
}: {
  profiles: ProfileRef[];
  active: string;
  busy?: boolean;
  onSwitch: (id: string) => void;
  onManage: () => void;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const current = profiles.find((p) => p.id === active) ?? profiles[0];

  useEffect(() => {
    if (!open) return;
    const onDown = (event: MouseEvent) => {
      if (!root.current?.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);

  if (!current) return null;

  return (
    <div className="profile-menu" ref={root}>
      <button
        type="button"
        className={`profile-chip${busy ? " busy" : ""}`}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
      >
        <span className="profile-chip-name">{current.name}</span>
        <span className="profile-chip-caret" aria-hidden>
          ▾
        </span>
      </button>
      {open ? (
        <div className="profile-dropdown" role="listbox">
          {profiles.map((profile) => (
            <button
              key={profile.id}
              type="button"
              role="option"
              aria-selected={profile.id === active}
              className={`profile-option${profile.id === active ? " on" : ""}`}
              onClick={() => {
                setOpen(false);
                if (profile.id !== active) onSwitch(profile.id);
              }}
            >
              <span className="profile-option-name">{profile.name}</span>
              {profile.id === active ? <TickIcon size={16} /> : null}
            </button>
          ))}
          <button
            type="button"
            className="profile-option manage"
            onClick={() => {
              setOpen(false);
              onManage();
            }}
          >
            <AddIcon size={16} />
            <span>Manage profiles…</span>
          </button>
        </div>
      ) : null}
    </div>
  );
}
