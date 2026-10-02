<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { findUpdate, type Release } from "$lib/updates";
  import { COLOR_NAMES, COLORS, vivid } from "$lib/notes";
  import { FONTS, saveSettings, settings, type Settings } from "$lib/settings.svelte";

  let { onclose }: { onclose: () => void } = $props();

  type MonitorInfo = { name: string; label: string };
  type GcalCalendar = { id: string; summary: string; color: string };
  type GcalStatus = { connected: boolean; last_sync: number | null; last_error: string | null };

  let monitors = $state<MonitorInfo[]>([]);
  let autostart = $state(false);
  let version = $state("");
  let error = $state("");
  let update_ = $state<Release | null>(null);
  let updateStatus = $state<"" | "checking" | "latest" | "failed">("");
  let dialog = $state<HTMLElement>();
  // Google Calendar
  let gcalStatus = $state<GcalStatus>({ connected: false, last_sync: null, last_error: null });
  let gcalCalendars = $state<GcalCalendar[]>([]);
  let gcalConnecting = $state(false);
  let gcalSyncing = $state(false);
  let gcalSyncResult = $state<string>("");
  // Non disturbare: fine del silenzio in corso (manuale o fascia oraria).
  let quietUntil = $state<number | null>(null);
  // Backup
  type BackupInfo = { name: string; created_at: number; notes: number; media_files: number };
  let backups = $state<BackupInfo[]>([]);
  let backupFolder = $state("");
  let backingUp = $state(false);
  let backupResult = $state("");
  let confirmRestore = $state<string | null>(null);
  let restoring = $state(false);

  const GCAL_RANGES: [string, string][] = [
    ["1w", "1 settimana"],
    ["2w", "2 settimane"],
    ["3w", "3 settimane"],
    ["1m", "1 mese"],
    ["2m", "2 mesi"],
    ["3m", "3 mesi"],
  ];

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
    ["Ctrl+Alt+D", "Non disturbare per un'ora / riattiva le notifiche"],
  ];

  async function update(changes: Partial<Settings>) {
    error = "";
    try {
      await saveSettings(changes);
    } catch (e) {
      error = String(e);
    }
  }

  async function checkUpdate() {
    updateStatus = "checking";
    try {
      update_ = await findUpdate(version || (await getVersion()));
      updateStatus = update_ ? "" : "latest";
    } catch (e) {
      console.error("controllo aggiornamenti", e);
      updateStatus = "failed";
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

  async function gcalConnect() {
    gcalConnecting = true;
    error = "";
    try {
      await invoke("gcal_connect");
      gcalCalendars = await invoke<GcalCalendar[]>("gcal_list_calendars");
    } catch (e) {
      error = `Google Calendar: ${e}`;
    } finally {
      gcalConnecting = false;
    }
  }

  async function gcalDisconnect() {
    try {
      await invoke("gcal_disconnect");
      gcalCalendars = [];
      await saveSettings({ gcal_enabled: false, gcal_calendar_ids: [] });
    } catch (e) {
      error = `Google Calendar: ${e}`;
    }
  }

  async function gcalSyncNow() {
    gcalSyncing = true;
    gcalSyncResult = "";
    try {
      const n = await invoke<number>("gcal_sync_now");
      gcalSyncResult = `${n} event${n === 1 ? "o" : "i"} importat${n === 1 ? "o" : "i"}`;
    } catch (e) {
      error = `Sync: ${e}`;
    } finally {
      gcalSyncing = false;
    }
  }

  function clock(unix: number): string {
    return new Date(unix * 1000).toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" });
  }

  function backupDate(unix: number): string {
    return new Date(unix * 1000).toLocaleString("it-IT", {
      weekday: "short",
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  // Si ricalcola quando cambia "Non disturbare" (anche dalla tray) o la fascia oraria.
  $effect(() => {
    void [settings.dnd_until, settings.quiet_enabled, settings.quiet_from, settings.quiet_to];
    invoke<number | null>("quiet_until").then((t) => (quietUntil = t));
  });

  async function setDnd(minutes: number | "morning" | null) {
    error = "";
    try {
      const until =
        minutes === null
          ? null
          : minutes === "morning"
            ? await invoke<number>("dnd_until_morning")
            : Math.floor(Date.now() / 1000) + minutes * 60;
      await invoke("set_dnd", { until });
    } catch (e) {
      error = `Non disturbare: ${e}`;
    }
  }

  async function loadBackups() {
    try {
      backupFolder = await invoke<string>("backup_folder");
      backups = await invoke<BackupInfo[]>("backup_list");
    } catch (e) {
      error = `Backup: ${e}`;
    }
  }

  async function backupNow() {
    backingUp = true;
    backupResult = "";
    error = "";
    try {
      const info = await invoke<BackupInfo>("backup_now");
      backupResult = `Fatto: ${info.notes} note salvate`;
      await loadBackups();
    } catch (e) {
      error = `Backup: ${e}`;
    } finally {
      backingUp = false;
    }
  }

  async function chooseBackupFolder() {
    const dir = await openDialog({ directory: true, title: "Cartella dei backup (es. su D: o in OneDrive)" });
    if (typeof dir === "string") {
      await update({ backup_dir: dir });
      await loadBackups();
    }
  }

  async function defaultBackupFolder() {
    await update({ backup_dir: null });
    await loadBackups();
  }

  /** Primo clic: chiede conferma; secondo clic: ripristina. */
  async function restore(name: string) {
    if (confirmRestore !== name) {
      confirmRestore = name;
      return;
    }
    confirmRestore = null;
    restoring = true;
    error = "";
    try {
      await invoke("backup_restore", { name });
      backupResult = "Note ripristinate. Lo stato di prima è in un nuovo backup.";
      await loadBackups();
    } catch (e) {
      error = `Ripristino: ${e}`;
    } finally {
      restoring = false;
    }
  }

  function toggleCalendar(id: string) {
    const ids = settings.gcal_calendar_ids.includes(id)
      ? settings.gcal_calendar_ids.filter((c) => c !== id)
      : [...settings.gcal_calendar_ids, id];
    update({ gcal_calendar_ids: ids });
  }

  onMount(() => {
    getVersion()
      .then((v) => {
        version = v;
        checkUpdate();
      })
      .catch(() => {});
    invoke<MonitorInfo[]>("list_monitors").then((list) => (monitors = list));
    invoke<boolean>("get_autostart").then((value) => (autostart = value));
    loadBackups();
    invoke<GcalStatus>("gcal_status").then((s) => {
      gcalStatus = s;
      if (s.connected) {
        invoke<GcalCalendar[]>("gcal_list_calendars")
          .then((cals) => (gcalCalendars = cals))
          .catch(() => {});
      }
    });
    dialog?.focus();
    // Cambiato dal menu della tray mentre il pannello è aperto.
    const unlisten = listen("autostart-changed", async () => (autostart = await invoke<boolean>("get_autostart")));
    const unlistenGcal = listen<GcalStatus>("gcal-status-changed", ({ payload }) => {
      gcalStatus = payload;
    });
    return () => {
      unlisten.then((fn) => fn());
      unlistenGcal.then((fn) => fn());
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
    <label class="row">
      <span>Ora di «Domani» quando posticipo</span>
      <input
        type="time"
        value={settings.tomorrow_time}
        onchange={(e) => e.currentTarget.value && update({ tomorrow_time: e.currentTarget.value })}
      />
    </label>
    <label class="row check">
      <span>Avvia con Windows</span>
      <input type="checkbox" checked={autostart} onchange={toggleAutostart} />
    </label>

    <h3>Non disturbare</h3>
    <div class="row">
      <span>
        {#if quietUntil}
          <strong>🌙 Notifiche in pausa fino alle {clock(quietUntil)}</strong>
          <small class="hint block">I promemoria arrivano tutti insieme alla fine.</small>
        {:else}
          Metti in pausa le notifiche dei promemoria
        {/if}
      </span>
      <div class="buttons">
        {#if settings.dnd_until}
          <button class="plain" onclick={() => setDnd(null)}>Riattiva</button>
        {:else}
          <button class="plain" onclick={() => setDnd(60)}>1 ora</button>
          <button class="plain" onclick={() => setDnd("morning")}>Fino a domattina</button>
        {/if}
      </div>
    </div>
    <label class="row check">
      <span>Ogni giorno in una fascia oraria</span>
      <input
        type="checkbox"
        checked={settings.quiet_enabled}
        onchange={(e) => update({ quiet_enabled: e.currentTarget.checked })}
      />
    </label>
    {#if settings.quiet_enabled}
      <div class="row">
        <span>Dalle … alle …</span>
        <div class="buttons">
          <input
            type="time"
            aria-label="Inizio"
            value={settings.quiet_from}
            onchange={(e) => e.currentTarget.value && update({ quiet_from: e.currentTarget.value })}
          />
          <span>→</span>
          <input
            type="time"
            aria-label="Fine"
            value={settings.quiet_to}
            onchange={(e) => e.currentTarget.value && update({ quiet_to: e.currentTarget.value })}
          />
        </div>
      </div>
    {/if}

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
    <label class="row check">
      <span>Backup automatico ogni giorno</span>
      <input
        type="checkbox"
        checked={settings.backup_enabled}
        onchange={(e) => update({ backup_enabled: e.currentTarget.checked })}
      />
    </label>
    <div class="row">
      <span class="path">
        Cartella dei backup
        <small class="hint block" title={backupFolder}>
          {settings.backup_dir ? backupFolder : "Predefinita (nella cartella dei dati)"}
        </small>
      </span>
      <div class="buttons">
        {#if settings.backup_dir}
          <button class="plain" onclick={defaultBackupFolder}>Predefinita</button>
        {/if}
        <button class="plain" onclick={chooseBackupFolder}>Cambia…</button>
      </div>
    </div>
    {#if !settings.backup_dir}
      <p class="hint">
        Meglio un altro disco o una cartella di OneDrive: se si guasta questo disco, i backup restano al sicuro.
      </p>
    {/if}
    <label class="row">
      <span>Backup da tenere</span>
      <select value={settings.backup_keep} onchange={(e) => update({ backup_keep: Number(e.currentTarget.value) })}>
        {#each [3, 7, 14, 30] as n (n)}
          <option value={n}>Gli ultimi {n}</option>
        {/each}
      </select>
    </label>
    <div class="row">
      <span>
        {backupResult || (backups[0] ? `Ultimo backup: ${backupDate(backups[0].created_at)}` : "Nessun backup ancora")}
      </span>
      <div class="buttons">
        <button class="plain" onclick={() => invoke("open_backup_folder")}>Apri</button>
        <button class="plain" disabled={backingUp || restoring} onclick={backupNow}>
          {backingUp ? "Backup…" : "Esegui ora"}
        </button>
      </div>
    </div>
    {#if backups.length > 0}
      <details class="backups">
        <summary>Ripristina da un backup ({backups.length})</summary>
        <p class="hint">
          Le note attuali vengono sostituite da quelle del backup. Prima Tabby salva lo stato attuale in un nuovo
          backup, così puoi sempre tornare indietro.
        </p>
        {#each backups as b (b.name)}
          <div class="backup-row">
            <span>{backupDate(b.created_at)} <small>{b.notes} note · {b.media_files} file</small></span>
            <button
              class="plain"
              class:danger={confirmRestore === b.name}
              disabled={restoring || backingUp}
              onclick={() => restore(b.name)}
            >
              {restoring && confirmRestore === null ? "…" : confirmRestore === b.name ? "Conferma ripristino" : "Ripristina"}
            </button>
          </div>
        {/each}
      </details>
    {/if}

    <h3>Google Calendar</h3>
    <label class="row check">
      <span>
        Usa il proxy di Windows
        <small class="hint" style="display: block; margin: 2px 0 0">Per reti aziendali con proxy: Tabby usa le impostazioni di Windows e le tue credenziali di accesso. Se spento, si collega a Google direttamente.</small>
      </span>
      <input
        type="checkbox"
        id="gcal-proxy-check"
        checked={settings.gcal_use_proxy}
        onchange={(e) => update({ gcal_use_proxy: e.currentTarget.checked })}
      />
    </label>
    {#if !gcalStatus.connected}
      <div class="row">
        <span>Collega il tuo account Google per importare gli eventi come note con promemoria.</span>
        <button
          id="gcal-connect-btn"
          class="plain gcal-btn"
          disabled={gcalConnecting}
          onclick={gcalConnect}
        >
          {gcalConnecting ? "Attendere…" : "🔗 Collega Google"}
        </button>
      </div>
    {:else}
      <div class="row">
        <span><strong>✓ Collegato</strong>{gcalStatus.last_sync ? ` · ultima sync ${new Date(gcalStatus.last_sync * 1000).toLocaleTimeString("it-IT", { hour: "2-digit", minute: "2-digit" })}` : ""}{gcalStatus.last_error ? ` · ⚠ ${gcalStatus.last_error}` : ""}</span>
        <button class="plain danger" onclick={gcalDisconnect}>Scollega</button>
      </div>
      <label class="row check">
        <span>Importazione attiva</span>
        <input
          type="checkbox"
          id="gcal-enabled-check"
          checked={settings.gcal_enabled}
          onchange={(e) => update({ gcal_enabled: e.currentTarget.checked })}
        />
      </label>
      {#if settings.gcal_enabled}
        <label class="row">
          <span>Importa gli eventi dei prossimi</span>
          <select value={settings.gcal_sync_range} onchange={(e) => update({ gcal_sync_range: e.currentTarget.value })}>
            {#each GCAL_RANGES as [key, label] (key)}
              <option value={key}>{label}</option>
            {/each}
          </select>
        </label>
        {#if gcalCalendars.length > 0}
          <div class="gcal-cals">
            <span class="gcal-cals-label">Calendari</span>
            {#each gcalCalendars as cal (cal.id)}
              <label class="gcal-cal-row">
                <input
                  type="checkbox"
                  checked={settings.gcal_calendar_ids.includes(cal.id)}
                  onchange={() => toggleCalendar(cal.id)}
                />
                <span class="gcal-dot" style:background={cal.color}></span>
                <span>{cal.summary}</span>
              </label>
            {/each}
          </div>
        {/if}
        <div class="row">
          <span>{gcalSyncResult || "Sincronizza subito gli eventi"}</span>
          <button
            id="gcal-sync-btn"
            class="plain"
            disabled={gcalSyncing || settings.gcal_calendar_ids.length === 0}
            onclick={gcalSyncNow}
          >
            {gcalSyncing ? "Sync…" : "↻ Sincronizza ora"}
          </button>
        </div>
      {/if}
    {/if}

    <h3>Informazioni</h3>
    <p class="about">
      <strong>Tabby</strong>{version ? ` ${version}` : ""}<br />
      Creato da <strong>Roberto Pisco Pisconti</strong>
    </p>
    <div class="row">
      <span>
        {#if update_}
          <strong>Nuova versione {update_.version} disponibile</strong>
        {:else if updateStatus === "checking"}
          Controllo aggiornamenti…
        {:else if updateStatus === "latest"}
          Hai l'ultima versione.
        {:else if updateStatus === "failed"}
          Non riesco a controllare gli aggiornamenti.
        {:else}
          Aggiornamenti
        {/if}
      </span>
      {#if update_}
        <button class="plain" onclick={() => update_ && openUrl(update_.url)}>Scarica</button>
      {:else}
        <button class="plain" disabled={updateStatus === "checking"} onclick={checkUpdate}>Controlla ora</button>
      {/if}
    </div>
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
  input[type="time"] {
    padding: 4px 6px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--pane);
    color: var(--text);
    font: inherit;
  }
  .buttons {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }
  .hint.block {
    display: block;
    margin: 2px 0 0;
  }
  .path {
    min-width: 0;
  }
  .path small {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .backups {
    padding: 6px 0;
    border-bottom: 1px solid var(--line);
  }
  .backups summary {
    padding: 4px 0;
    cursor: pointer;
  }
  .backup-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 0;
  }
  .plain:disabled {
    opacity: 0.55;
    cursor: default;
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
  .gcal-btn {
    white-space: nowrap;
    flex-shrink: 0;
  }
  .danger {
    color: var(--danger, #e55);
    border-color: color-mix(in srgb, var(--danger, #e55) 40%, transparent);
  }
  .danger:hover {
    background: color-mix(in srgb, var(--danger, #e55) 10%, transparent);
  }
  .gcal-cals {
    padding: 6px 0 4px;
    border-bottom: 1px solid var(--line);
  }
  .gcal-cals-label {
    display: block;
    margin-bottom: 6px;
    color: var(--muted);
    font-size: 12px;
  }
  .gcal-cal-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 4px 0;
    cursor: pointer;
  }
  .gcal-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    flex-shrink: 0;
  }
</style>
