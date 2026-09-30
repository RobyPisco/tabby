<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { COLOR_NAMES, COLORS, vivid } from "$lib/notes";
  import { FONTS, saveSettings, settings, type Settings } from "$lib/settings.svelte";

  let { onclose }: { onclose: () => void } = $props();

  type MonitorInfo = { name: string; label: string };
  let monitors = $state<MonitorInfo[]>([]);
  let autostart = $state(false);
  let version = $state("");
  let error = $state("");
  let dialog = $state<HTMLElement>();

  const SOUNDS: [string, string][] = [
    ["reminder", "Promemoria"],
    ["default", "Classico"],
    ["im", "Messaggio"],
    ["mail", "Posta"],
    ["sms", "SMS"],
    ["alarm", "Sveglia"],
    ["alarm2", "Sveglia 2"],
    ["call", "Chiamata"],
    ["call2", "Chiamata 2"],
  ];

  const SHORTCUTS: [string, string][] = [
    ["Ctrl+Alt+L", "Tutte le note"],
    ["Ctrl+Alt+N", "Nuova nota nel deck"],
    ["Ctrl+Alt+P", "Nuova nota con promemoria"],
    ["Ctrl+Alt+V", "Salva gli appunti come nota"],
    ["Ctrl+Alt+H", "Mostra/nascondi il deck"],
  ];

  async function update(changes: Partial<Settings>) {
    error = "";
    try {
      await saveSettings(changes);
    } catch (e) {
      error = String(e);
    }
  }

  async function toggleAutostart() {
    try {
      autostart = await invoke<boolean>("set_autostart", { enabled: !autostart });
    } catch (e) {
      error = `Avvio automatico: ${e}`;
      autostart = await invoke<boolean>("get_autostart");
    }
  }

  onMount(() => {
    getVersion().then((v) => (version = v)).catch(() => {});
    invoke<MonitorInfo[]>("list_monitors").then((list) => (monitors = list));
    invoke<boolean>("get_autostart").then((value) => (autostart = value));
    dialog?.focus();
    // Cambiato dal menu della tray mentre il pannello è aperto.
    const unlisten = listen("autostart-changed", async () => (autostart = await invoke<boolean>("get_autostart")));
    return () => {
      unlisten.then((fn) => fn());
    };
  });
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label="Impostazioni"
    tabindex="-1"
    bind:this={dialog}
    onkeydown={(e) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onclose();
      }
    }}
  >
    <header>
      <h2>Impostazioni</h2>
      <button class="close" onclick={onclose} aria-label="Chiudi le impostazioni">✕</button>
    </header>

    {#if error}<p class="error">{error}</p>{/if}

    <h3>Deck</h3>
    <div class="row">
      <span>Bordo dello schermo</span>
      <div class="segmented">
        <button class:on={settings.side === "left"} onclick={() => update({ side: "left" })}>Sinistra</button>
        <button class:on={settings.side === "right"} onclick={() => update({ side: "right" })}>Destra</button>
      </div>
    </div>
    <label class="row">
      <span>Larghezza della nota <small>{settings.note_width} px</small></span>
      <input
        type="range"
        min="280"
        max="520"
        step="20"
        value={settings.note_width}
        onchange={(e) => update({ note_width: Number(e.currentTarget.value) })}
      />
    </label>
    <label class="row">
      <span>Trasparenza delle schede <small>{settings.deck_transparency}%</small></span>
      <input
        type="range"
        min="0"
        max="100"
        step="5"
        value={settings.deck_transparency}
        onchange={(e) => update({ deck_transparency: Number(e.currentTarget.value) })}
      />
    </label>
    {#if monitors.length > 1}
      <label class="row">
        <span>Schermo</span>
        <select
          value={settings.monitor ?? ""}
          onchange={(e) => update({ monitor: e.currentTarget.value || null })}
        >
          <option value="">Principale</option>
          {#each monitors as m (m.name)}
            <option value={m.name}>{m.label}</option>
          {/each}
        </select>
      </label>
    {/if}

    <h3>Testo delle note</h3>
    <label class="row">
      <span>Carattere</span>
      <select
        value={settings.editor_font}
        onchange={(e) => update({ editor_font: e.currentTarget.value as Settings["editor_font"] })}
      >
        {#each Object.entries(FONTS) as [key, font] (key)}
          <option value={key}>{font.label}</option>
        {/each}
      </select>
    </label>
    <label class="row">
      <span>Dimensione <small>{settings.editor_size} px</small></span>
      <input
        type="range"
        min="13"
        max="24"
        value={settings.editor_size}
        onchange={(e) => update({ editor_size: Number(e.currentTarget.value) })}
      />
    </label>
    <div class="row">
      <span>Colore delle note nuove</span>
      <div class="colors">
        <button
          class="cycle"
          class:on={settings.new_note_color === "cycle"}
          onclick={() => update({ new_note_color: "cycle" })}
        >A rotazione</button>
        {#each COLORS as c (c)}
          <button
            class="swatch"
            class:on={settings.new_note_color === c}
            style:background={c}
            style:--ring={vivid(c)}
            onclick={() => update({ new_note_color: c })}
            aria-label={COLOR_NAMES[c]}
            title={COLOR_NAMES[c]}
          ></button>
        {/each}
      </div>
    </div>

    <h3>Aspetto e avvio</h3>
    <div class="row">
      <span>Tema di questa finestra</span>
      <div class="segmented">
        <button class:on={settings.theme === "auto"} onclick={() => update({ theme: "auto" })}>Automatico</button>
        <button class:on={settings.theme === "light"} onclick={() => update({ theme: "light" })}>Chiaro</button>
        <button class:on={settings.theme === "dark"} onclick={() => update({ theme: "dark" })}>Scuro</button>
      </div>
    </div>
    <label class="row check">
      <span>Suono delle notifiche</span>
      <input type="checkbox" checked={settings.sound} onchange={(e) => update({ sound: e.currentTarget.checked })} />
    </label>
    {#if settings.sound}
      <div class="row">
        <span>Quale suono</span>
        <div class="sound-pick">
          <select value={settings.sound_name} onchange={(e) => update({ sound_name: e.currentTarget.value })}>
            {#each SOUNDS as [key, label] (key)}
              <option value={key}>{label}</option>
            {/each}
          </select>
          <button class="plain" onclick={() => invoke("preview_sound", { name: settings.sound_name })}>▶ Prova</button>
        </div>
      </div>
      <label class="row">
        <span>Ripeti se non rispondo</span>
        <select
          value={settings.repeat_minutes}
          onchange={(e) => update({ repeat_minutes: Number(e.currentTarget.value) })}
        >
          <option value={0}>Mai</option>
          {#each [1, 2, 5, 10, 15, 30] as m (m)}
            <option value={m}>Ogni {m} min</option>
          {/each}
        </select>
      </label>
    {/if}
    <label class="row check">
      <span>Avvia con Windows</span>
      <input type="checkbox" checked={autostart} onchange={toggleAutostart} />
    </label>

    <h3>Scorciatoie</h3>
    <dl>
      {#each SHORTCUTS as [keys, action] (keys)}
        <dt><kbd>{keys}</kbd></dt>
        <dd>{action}</dd>
      {/each}
    </dl>
    <p class="hint">
      Se un'altra app usa già una di queste combinazioni, quella scorciatoia non è disponibile.
    </p>

    <h3>Dati</h3>
    <div class="row">
      <span>Note, immagini e impostazioni sono salvate sul PC.</span>
      <button class="plain" onclick={() => invoke("open_data_folder")}>Apri la cartella</button>
    </div>

    <h3>Informazioni</h3>
    <p class="about">
      <strong>Tabby</strong>{version ? ` ${version}` : ""}<br />
      Creato da <strong>Roberto Pisco Pisconti</strong>
    </p>
  </div>
</div>

<style>
  button {
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
  }
  .dialog {
    width: min(560px, calc(100vw - 40px));
    max-height: calc(100vh - 60px);
    overflow-y: auto;
    box-sizing: border-box;
    padding: 18px 22px 22px;
    border-radius: 16px;
    background: var(--pane);
    color: var(--text);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
    outline: none;
  }
  .dialog::-webkit-scrollbar {
    width: 8px;
  }
  .dialog::-webkit-scrollbar-button {
    display: none;
  }
  .dialog::-webkit-scrollbar-thumb {
    border: 2px solid var(--pane);
    border-radius: 4px;
    background: var(--line);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 18px;
  }
  h3 {
    margin: 18px 0 6px;
    color: var(--muted);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .close {
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 8px;
    background: transparent;
  }
  .close:hover {
    background: var(--hover);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 38px;
    padding: 4px 0;
    border-bottom: 1px solid var(--line);
  }
  .row > span {
    flex: 1;
  }
  small {
    margin-left: 6px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  select {
    padding: 5px 8px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--pane);
    color: var(--text);
    font: inherit;
  }
  input[type="range"] {
    width: 180px;
    accent-color: var(--focus);
  }
  input[type="checkbox"] {
    width: 18px;
    height: 18px;
    accent-color: var(--focus);
  }
  .sound-pick {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .segmented {
    display: flex;
    gap: 2px;
    padding: 2px;
    border-radius: 8px;
    background: var(--hover);
  }
  .segmented button {
    padding: 4px 10px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: var(--muted);
    font-weight: 600;
  }
  .segmented button.on {
    background: var(--pane);
    color: var(--text);
    box-shadow: 0 1px 2px rgba(0, 0, 0, 0.15);
  }
  .colors {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .cycle {
    padding: 3px 8px;
    border: 1px solid var(--line);
    border-radius: 999px;
    background: var(--pane);
    font-size: 12px;
  }
  .cycle.on {
    border-color: var(--focus);
    background: var(--focus);
    color: #fff;
  }
  .swatch {
    width: 20px;
    height: 20px;
    padding: 0;
    border: 2px solid var(--ring);
    border-radius: 50%;
  }
  .swatch.on {
    box-shadow:
      0 0 0 2px var(--pane),
      0 0 0 4px var(--ring);
  }
  dl {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 14px;
    margin: 4px 0;
  }
  dd {
    margin: 0;
  }
  kbd {
    padding: 1px 6px;
    border: 1px solid var(--line);
    border-radius: 5px;
    background: var(--hover);
    font: 600 12px "Cascadia Mono", Consolas, monospace;
  }
  .plain {
    padding: 5px 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--pane);
  }
  .plain:hover {
    background: var(--hover);
  }
  .about {
    margin: 4px 0 0;
    line-height: 1.5;
  }
  .hint {
    margin: 6px 0 0;
    color: var(--muted);
    font-size: 12px;
  }
  .error {
    margin: 10px 0 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 15%, transparent);
    color: var(--danger);
  }
</style>
