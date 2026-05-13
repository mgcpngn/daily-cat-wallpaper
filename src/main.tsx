import React, { useEffect, useMemo, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import "./styles.css";

type CatImageType = "Healing" | "Funny" | "Loaf" | "Kitten" | "Sleepy";
type PlatformMode = "Automatic" | "StaticOnly" | "InteractionBeta";
type ScheduleConfig =
  | "OnLogin"
  | "ManualOnly"
  | { Daily: { time: string } }
  | { EveryHours: { hours: number } };

type AppConfig = {
  breeds: string[];
  cat_count: number;
  image_types: CatImageType[];
  interactions: {
    breathing: boolean;
    mouse_proximity: boolean;
    click_paw: boolean;
    keyboard_bongo: boolean;
    sound: boolean;
  };
  schedule: ScheduleConfig;
  sources: {
    local_dirs: string[];
    cataas: boolean;
    the_cat_api: boolean;
    pexels_api_key: string | null;
    unsplash_access_key: string | null;
  };
  platform_mode: PlatformMode;
  launch_at_login: boolean;
};

type Capabilities = {
  platform: string;
  static_wallpaper: boolean;
  interaction_overlay: boolean;
  supported_interactions: string[];
  beta: boolean;
};

type Rect = {
  x: number;
  y: number;
  width: number;
  height: number;
};

const BREEDS = [
  "mixed",
  "orange tabby",
  "british shorthair",
  "ragdoll",
  "maine coon",
  "siamese",
  "black cat",
  "calico",
  "white cat",
];

const IMAGE_TYPES: Array<{ key: CatImageType; label: string }> = [
  { key: "Healing", label: "Healing" },
  { key: "Funny", label: "Funny" },
  { key: "Loaf", label: "Loaf" },
  { key: "Kitten", label: "Kitten" },
  { key: "Sleepy", label: "Sleepy" },
];

const defaultConfig: AppConfig = {
  breeds: ["mixed"],
  cat_count: 1,
  image_types: ["Healing", "Loaf"],
  interactions: {
    breathing: true,
    mouse_proximity: true,
    click_paw: false,
    keyboard_bongo: false,
    sound: false,
  },
  schedule: { Daily: { time: "09:00" } },
  sources: {
    local_dirs: [],
    cataas: true,
    the_cat_api: true,
    pexels_api_key: null,
    unsplash_access_key: null,
  },
  platform_mode: "Automatic",
  launch_at_login: true,
};

function App() {
  const [config, setConfig] = useState<AppConfig>(defaultConfig);
  const [capabilities, setCapabilities] = useState<Capabilities | null>(null);
  const [slots, setSlots] = useState<Rect[]>([]);
  const [status, setStatus] = useState("Loading configuration");
  const [localDirInput, setLocalDirInput] = useState("");
  const scheduleKind = getScheduleKind(config.schedule);

  useEffect(() => {
    Promise.all([
      invoke<AppConfig>("get_config"),
      invoke<Capabilities>("platform_capabilities"),
    ])
      .then(([loadedConfig, loadedCapabilities]) => {
        setConfig(loadedConfig);
        setCapabilities(loadedCapabilities);
        setStatus("Ready");
      })
      .catch((error) => setStatus(String(error)));
  }, []);

  useEffect(() => {
    invoke<Rect[]>("preview_layout", {
      catCount: config.cat_count,
      width: 1920,
      height: 1080,
    })
      .then(setSlots)
      .catch(() => setSlots([]));
  }, [config.cat_count]);

  const enabledInteractions = useMemo(
    () =>
      Object.entries(config.interactions)
        .filter(([, enabled]) => enabled)
        .map(([key]) => labelize(key)),
    [config.interactions],
  );

  async function save() {
    setStatus("Saving preferences");
    try {
      const saved = await invoke<AppConfig>("save_config", { config });
      setConfig(saved);
      setStatus("Preferences saved");
    } catch (error) {
      setStatus(String(error));
    }
  }

  async function refreshNow() {
    setStatus("Refreshing wallpaper");
    try {
      const path = await invoke<string>("refresh_wallpaper");
      setStatus(`Wallpaper set: ${path}`);
    } catch (error) {
      setStatus(String(error));
    }
  }

  return (
    <main className="app-shell">
      <section className="hero">
        <div>
          <p className="eyebrow">Daily Cat Wallpaper</p>
          <h1>Cat-powered desktop control.</h1>
          <p className="lead">
            Pick the cats, frequency, and interaction style. The Rust core handles
            wallpaper updates while this panel keeps preferences clear.
          </p>
          <div className="action-row">
            <button className="primary" onClick={refreshNow}>
              Refresh now
            </button>
            <button className="secondary" onClick={save}>
              Save preferences
            </button>
          </div>
        </div>
        <div className="status-panel" aria-live="polite">
          <span className="status-label">Status</span>
          <strong>{status}</strong>
          <span>
            {capabilities
              ? `${capabilities.platform}${capabilities.beta ? " beta" : ""} / static ${
                  capabilities.static_wallpaper ? "ready" : "unavailable"
                }`
              : "Detecting platform"}
          </span>
        </div>
      </section>

      <section className="workbench">
        <aside className="preview-pane">
          <div className="screen-preview">
            {slots.map((slot, index) => (
              <div
                className="cat-slot"
                key={`${slot.x}-${slot.y}-${index}`}
                style={{
                  left: `${(slot.x / 1920) * 100}%`,
                  top: `${(slot.y / 1080) * 100}%`,
                  width: `${(slot.width / 1920) * 100}%`,
                  height: `${(slot.height / 1080) * 100}%`,
                }}
              >
                <span>{index + 1}</span>
              </div>
            ))}
          </div>
          <div className="preview-copy">
            <h2>{config.cat_count} cats on screen</h2>
            <p>
              Safe layout keeps the left icon area and taskbar clear. Active:
              {" "}
              {enabledInteractions.length ? enabledInteractions.join(", ") : "none"}.
            </p>
          </div>
        </aside>

        <section className="settings-grid">
          <Panel title="Cat identity">
            <label className="field">
              Breed preferences
              <select
                multiple
                value={config.breeds}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    breeds: Array.from(event.currentTarget.selectedOptions).map(
                      (option) => option.value,
                    ),
                  })
                }
              >
                {BREEDS.map((breed) => (
                  <option key={breed} value={breed}>
                    {breed}
                  </option>
                ))}
              </select>
            </label>
            <label className="field">
              Same-screen cat count
              <input
                type="range"
                min="1"
                max="5"
                value={config.cat_count}
                onChange={(event) =>
                  setConfig({ ...config, cat_count: Number(event.currentTarget.value) })
                }
              />
              <span className="range-value">{config.cat_count}</span>
            </label>
          </Panel>

          <Panel title="Image mood">
            <div className="chips">
              {IMAGE_TYPES.map((type) => (
                <button
                  className={config.image_types.includes(type.key) ? "chip active" : "chip"}
                  key={type.key}
                  onClick={() =>
                    setConfig({
                      ...config,
                      image_types: toggleArray(config.image_types, type.key),
                    })
                  }
                  type="button"
                >
                  {type.label}
                </button>
              ))}
            </div>
          </Panel>

          <Panel title="Interactions">
            {Object.entries(config.interactions).map(([key, enabled]) => (
              <label className="switch" key={key}>
                <span>
                  <strong>{labelize(key)}</strong>
                  <small>{interactionHint(key)}</small>
                </span>
                <input
                  type="checkbox"
                  checked={enabled}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      interactions: {
                        ...config.interactions,
                        [key]: event.currentTarget.checked,
                      },
                    })
                  }
                />
              </label>
            ))}
          </Panel>

          <Panel title="Refresh frequency">
            <div className="segmented">
              {["Daily", "EveryHours", "OnLogin", "ManualOnly"].map((kind) => (
                <button
                  className={scheduleKind === kind ? "active" : ""}
                  key={kind}
                  onClick={() => setConfig({ ...config, schedule: scheduleForKind(kind) })}
                  type="button"
                >
                  {scheduleLabel(kind)}
                </button>
              ))}
            </div>
            {scheduleKind === "Daily" && (
              <label className="field">
                Daily time
                <input
                  type="time"
                  value={dailyTime(config.schedule)}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      schedule: { Daily: { time: event.currentTarget.value } },
                    })
                  }
                />
              </label>
            )}
            {scheduleKind === "EveryHours" && (
              <label className="field">
                Every N hours
                <input
                  type="number"
                  min="1"
                  max="24"
                  value={intervalHours(config.schedule)}
                  onChange={(event) =>
                    setConfig({
                      ...config,
                      schedule: {
                        EveryHours: { hours: Number(event.currentTarget.value) },
                      },
                    })
                  }
                />
              </label>
            )}
          </Panel>

          <Panel title="Sources">
            <label className="switch">
              <span>
                <strong>CATAAS</strong>
                <small>Random no-key cat images</small>
              </span>
              <input
                type="checkbox"
                checked={config.sources.cataas}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    sources: { ...config.sources, cataas: event.currentTarget.checked },
                  })
                }
              />
            </label>
            <label className="switch">
              <span>
                <strong>TheCatAPI</strong>
                <small>Random cats and breed metadata</small>
              </span>
              <input
                type="checkbox"
                checked={config.sources.the_cat_api}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    sources: {
                      ...config.sources,
                      the_cat_api: event.currentTarget.checked,
                    },
                  })
                }
              />
            </label>
            <div className="inline-field">
              <input
                placeholder="C:\\Users\\you\\Pictures\\Cats"
                value={localDirInput}
                onChange={(event) => setLocalDirInput(event.currentTarget.value)}
              />
              <button
                type="button"
                onClick={() => {
                  if (!localDirInput.trim()) return;
                  setConfig({
                    ...config,
                    sources: {
                      ...config.sources,
                      local_dirs: [...config.sources.local_dirs, localDirInput.trim()],
                    },
                  });
                  setLocalDirInput("");
                }}
              >
                Add
              </button>
            </div>
            <div className="source-list">
              {config.sources.local_dirs.map((dir) => (
                <button
                  key={dir}
                  type="button"
                  onClick={() =>
                    setConfig({
                      ...config,
                      sources: {
                        ...config.sources,
                        local_dirs: config.sources.local_dirs.filter((item) => item !== dir),
                      },
                    })
                  }
                >
                  {dir}
                </button>
              ))}
            </div>
          </Panel>

          <Panel title="Platform">
            <label className="field">
              Mode
              <select
                value={config.platform_mode}
                onChange={(event) =>
                  setConfig({
                    ...config,
                    platform_mode: event.currentTarget.value as PlatformMode,
                  })
                }
              >
                <option value="Automatic">Automatic</option>
                <option value="StaticOnly">Static only</option>
                <option value="InteractionBeta">Interaction beta</option>
              </select>
            </label>
            <label className="switch">
              <span>
                <strong>Launch at login</strong>
                <small>Refresh cats when the desktop starts</small>
              </span>
              <input
                type="checkbox"
                checked={config.launch_at_login}
                onChange={(event) =>
                  setConfig({ ...config, launch_at_login: event.currentTarget.checked })
                }
              />
            </label>
          </Panel>
        </section>
      </section>
    </main>
  );
}

function Panel({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="panel">
      <h2>{title}</h2>
      {children}
    </section>
  );
}

function toggleArray<T>(items: T[], item: T): T[] {
  return items.includes(item) ? items.filter((value) => value !== item) : [...items, item];
}

function labelize(value: string): string {
  return value
    .split("_")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function interactionHint(key: string): string {
  const hints: Record<string, string> = {
    breathing: "Subtle motion on generated cat layers",
    mouse_proximity: "Cats react when the pointer gets close",
    click_paw: "Click feedback for cat paws",
    keyboard_bongo: "Keyboard rhythm reaction",
    sound: "Optional short sound effects",
  };
  return hints[key] ?? "Optional behavior";
}

function getScheduleKind(schedule: ScheduleConfig): string {
  if (typeof schedule === "string") return schedule;
  if ("Daily" in schedule) return "Daily";
  return "EveryHours";
}

function dailyTime(schedule: ScheduleConfig): string {
  return typeof schedule === "object" && "Daily" in schedule ? schedule.Daily.time : "09:00";
}

function intervalHours(schedule: ScheduleConfig): number {
  return typeof schedule === "object" && "EveryHours" in schedule
    ? schedule.EveryHours.hours
    : 4;
}

function scheduleForKind(kind: string): ScheduleConfig {
  switch (kind) {
    case "OnLogin":
      return "OnLogin";
    case "ManualOnly":
      return "ManualOnly";
    case "EveryHours":
      return { EveryHours: { hours: 4 } };
    default:
      return { Daily: { time: "09:00" } };
  }
}

function scheduleLabel(kind: string): string {
  const labels: Record<string, string> = {
    Daily: "Daily",
    EveryHours: "Every N hours",
    OnLogin: "On login",
    ManualOnly: "Manual",
  };
  return labels[kind] ?? kind;
}

createRoot(document.getElementById("root")!).render(<App />);
