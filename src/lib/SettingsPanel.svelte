<script module lang="ts">
  let lastTab: "general" | "notes" | "reminders" | "sync" | "backup" | "about" = "general";
</script>

<script lang="ts">
  import { locale, plural, tr } from "$lib/i18n";
  import { invoke } from "@tauri-apps/api/core";
  import { getVersion } from "@tauri-apps/api/app";
  import { listen } from "@tauri-apps/api/event";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { onMount } from "svelte";
  import { findUpdate, type Release } from "$lib/updates";
  import { COLOR_NAMES, COLORS, vivid } from "$lib/notes";
  import { FONTS, hasPin, saveSettings, settings, type Settings } from "$lib/settings.svelte";

  let { onclose }: { onclose: () => void } = $props();

  type MonitorInfo = { name: string; label: string };
  type DeviceInfo = { name: string; seen_at: number; this_pc: boolean };
  type ShareStatus = {
    folder_uid: string;
    name: string;
    dir: string;
    last_sync: number | null;
    last_error: string | null;
    devices: DeviceInfo[];
  };
  type SyncStatus = {
    dir: string | null;
    last_sync: number | null;
    last_error: string | null;
    devices: DeviceInfo[];
    encrypted: boolean;
    needs_passphrase: boolean;
    phone_copy: boolean;
    service: string | null;
    shares: ShareStatus[];
  };
  type CloudFolder = { service: string; path: string };
  type FolderInfo = { id: number; name: string; count: number };

  let monitors = $state<MonitorInfo[]>([]);
  let autostart = $state(false);
  /** Installato dal Microsoft Store: aggiornamenti e avvio con Windows li gestisce Windows. */
  let storeBuild = $state(false);
  let version = $state("");
  let error = $state("");
  let update_ = $state<Release | null>(null);
  let updateStatus = $state<"" | "checking" | "latest" | "failed" | "store">("");
  let dialog = $state<HTMLElement>();
  // Sincronizzazione tra PC
  let syncStatus = $state<SyncStatus>({
    dir: null,
    last_sync: null,
    last_error: null,
    devices: [],
    encrypted: false,
    needs_passphrase: false,
    phone_copy: true,
    service: null,
    shares: [],
  });
  let clouds = $state<CloudFolder[]>([]);
  let syncConfirmOff = $state(false);
  let passphrase = $state("");
  let passphraseOpen = $state(false);
  let passphraseBusy = $state(false);
  let confirmDecrypt = $state(false);
  let folders = $state<FolderInfo[]>([]);
  let shareFolderId = $state<number | null>(null);
  let confirmLeave = $state<string | null>(null);
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
  // Protezione con PIN
  let changingPin = $state(false);
  let oldPin = $state("");
  let newPin = $state("");
  let newPinConfirm = $state("");
  let confirmResetPin = $state(false);

  type Tab = "general" | "notes" | "reminders" | "sync" | "backup" | "about";
  const TABS: { id: Tab; icon: string; label: string; intro: string }[] = [
    { id: "general", icon: "⚙️", get label() { return tr("Generale", "General"); }, get intro() { return tr("Avvio, tema, lingua e posizione del deck.", "Startup, theme, language and deck position."); } },
    { id: "notes", icon: "📝", get label() { return tr("Note", "Notes"); }, get intro() { return tr("Come appaiono le note.", "How notes look."); } },
    { id: "reminders", icon: "🔔", get label() { return tr("Promemoria", "Reminders"); }, get intro() { return tr("Notifiche, suoni e momenti di silenzio.", "Notifications, sounds and quiet times."); } },
    { id: "sync", icon: "🔄", get label() { return tr("Sincronizzazione", "Sync"); }, get intro() { return tr("Le stesse note su tutti i tuoi PC, tramite Google Drive, OneDrive o Dropbox.", "The same notes on all your PCs, through Google Drive, OneDrive or Dropbox."); } },
    { id: "backup", icon: "💾", get label() { return tr("Backup e dati", "Backup and data"); }, get intro() { return tr("Copie di sicurezza delle note e dove sono salvate.", "Safety copies of your notes and where they are stored."); } },
    { id: "about", icon: "ℹ️", get label() { return tr("Informazioni", "About"); }, get intro() { return tr("Versione, aggiornamenti e scorciatoie.", "Version, updates and shortcuts."); } },
  ];
  // Riapre il pannello sull'ultima sezione vista.
  let tab = $state<Tab>(lastTab);
  const current = $derived(TABS.find((t) => t.id === tab)!);
  $effect(() => {
    lastTab = tab;
  });

  const SOUNDS = $derived<[string, string][]>([
    ["reminder", tr("Promemoria", "Reminder")],
    ["default", tr("Classico", "Classic")],
    ["im", tr("Messaggio", "Message")],
    ["mail", tr("Posta", "Mail")],
    ["sms", "SMS"],
    ["alarm", tr("Sveglia", "Alarm")],
    ["alarm2", tr("Sveglia 2", "Alarm 2")],
    ["call", tr("Chiamata", "Call")],
    ["call2", tr("Chiamata 2", "Call 2")],
  ]);

  const SHORTCUTS = $derived<[string, string][]>([
    ["Ctrl+Alt+L", tr("Tutte le note", "All notes")],
    ["Ctrl+Alt+N", tr("Nuova nota nel deck", "New note in the deck")],
    ["Ctrl+Alt+P", tr("Nuova nota con promemoria", "New note with reminder")],
    ["Ctrl+Alt+V", tr("Salva gli appunti come nota", "Save the clipboard as a note")],
    ["Ctrl+Alt+H", tr("Mostra/nascondi il deck", "Show/hide the deck")],
    ["Ctrl+Alt+D", tr("Non disturbare per un'ora / riattiva le notifiche", "Do not disturb for an hour / turn notifications back on")],
    ["Ctrl+Alt+F", tr("Cerca ovunque: trova una nota e aprila al volo", "Search everywhere: find a note and open it right away")],
  ]);

  async function update(changes: Partial<Settings>) {
    error = "";
    try {
      await saveSettings(changes);
    } catch (e) {
      error = String(e);
    }
  }

  async function submitChangePin() {
    error = "";
    if (newPin !== newPinConfirm) {
      error = tr("I due PIN non coincidono", "The two PINs don't match");
      return;
    }
    try {
      await invoke("change_pin", { oldPin, newPin });
      changingPin = false;
      oldPin = newPin = newPinConfirm = "";
    } catch (e) {
      error = String(e);
    }
  }

  async function resetPin() {
    if (!confirmResetPin) {
      confirmResetPin = true;
      return;
    }
    error = "";
    try {
      await invoke("reset_pin");
      confirmResetPin = false;
    } catch (e) {
      error = String(e);
    }
  }

  async function checkUpdate() {
    updateStatus = "checking";
    try {
      if (storeBuild || (await invoke<boolean>("is_store_build"))) {
        updateStatus = "store";
        return;
      }
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
      error = `${tr("Avvio automatico", "Autostart")}: ${e}`;
      autostart = await invoke<boolean>("get_autostart");
    }
  }

  function clock(unix: number): string {
    return new Date(unix * 1000).toLocaleTimeString(locale(), { hour: "2-digit", minute: "2-digit" });
  }

  function backupDate(unix: number): string {
    return new Date(unix * 1000).toLocaleString(locale(), {
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
      error = `${tr("Non disturbare", "Do not disturb")}: ${e}`;
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
      backupResult = `${tr("Fatto", "Done")}: ${plural(info.notes, ["nota salvata", "note salvate"], ["note saved", "notes saved"])}`;
      await loadBackups();
    } catch (e) {
      error = `Backup: ${e}`;
    } finally {
      backingUp = false;
    }
  }

  async function chooseSyncFolder() {
    const dir = await openDialog({ directory: true, title: tr("Cartella condivisa (es. Il mio Drive, OneDrive o Dropbox)", "Shared folder (e.g. My Drive, OneDrive or Dropbox)") });
    if (typeof dir !== "string") return;
    error = "";
    try {
      await invoke<string>("sync_choose_folder", { path: dir });
      syncStatus = await invoke<SyncStatus>("sync_status");
    } catch (e) {
      error = `${tr("Sincronizzazione", "Sync")}: ${e}`;
    }
  }

  /** Usa la cartella di un servizio cloud trovato sul PC (Tabby ci crea "Tabby Sync"). */
  async function useCloud(path: string) {
    error = "";
    try {
      await invoke<string>("sync_choose_folder", { path });
      setTimeout(refreshSync, 1500);
    } catch (e) {
      error = `${tr("Sincronizzazione", "Sync")}: ${e}`;
    }
  }

  const DOWNLOADS: [string, string][] = [
    ["Google Drive", "https://www.google.com/drive/download/"],
    ["Dropbox", "https://www.dropbox.com/install"],
  ];

  async function syncDisconnect() {
    if (!syncConfirmOff) {
      syncConfirmOff = true;
      return;
    }
    syncConfirmOff = false;
    try {
      await invoke("sync_disconnect");
    } catch (e) {
      error = `${tr("Sincronizzazione", "Sync")}: ${e}`;
    }
  }

  async function refreshSync() {
    syncStatus = await invoke<SyncStatus>("sync_status");
  }

  async function savePassphrase() {
    passphraseBusy = true;
    error = "";
    try {
      await invoke("sync_set_passphrase", { passphrase });
      passphrase = "";
      passphraseOpen = false;
      setTimeout(refreshSync, 1500);
    } catch (e) {
      error = `${tr("Frase segreta", "Passphrase")}: ${e}`;
    } finally {
      passphraseBusy = false;
    }
  }

  async function removeEncryption() {
    if (!confirmDecrypt) {
      confirmDecrypt = true;
      return;
    }
    confirmDecrypt = false;
    try {
      await invoke("sync_remove_encryption");
      setTimeout(refreshSync, 1500);
    } catch (e) {
      error = `${tr("Cifratura", "Encryption")}: ${e}`;
    }
  }

  async function setPhoneCopy(enabled: boolean) {
    try {
      await invoke("sync_set_phone_copy", { enabled });
      syncStatus.phone_copy = enabled;
    } catch (e) {
      error = `${tr("Copie per il telefono", "Phone copies")}: ${e}`;
    }
  }

  async function shareFolder() {
    if (shareFolderId === null) return;
    const dir = await openDialog({ directory: true, title: tr("Cartella di Drive (o OneDrive, Dropbox) da condividere con le altre persone", "Drive (or OneDrive, Dropbox) folder to share with the other people") });
    if (typeof dir !== "string") return;
    error = "";
    try {
      await invoke<string>("share_folder", { folderId: shareFolderId, path: dir });
      shareFolderId = null;
      setTimeout(refreshSync, 1500);
    } catch (e) {
      error = `${tr("Lista condivisa", "Shared list")}: ${e}`;
    }
  }

  async function joinShare() {
    const dir = await openDialog({ directory: true, title: tr("Cartella che ti hanno condiviso", "Folder that was shared with you") });
    if (typeof dir !== "string") return;
    error = "";
    try {
      await invoke<string>("share_join", { path: dir });
      setTimeout(refreshSync, 1500);
    } catch (e) {
      error = `${tr("Lista condivisa", "Shared list")}: ${e}`;
    }
  }

  async function leaveShare(uid: string) {
    if (confirmLeave !== uid) {
      confirmLeave = uid;
      return;
    }
    confirmLeave = null;
    try {
      await invoke("share_leave", { folderUid: uid });
      setTimeout(refreshSync, 500);
    } catch (e) {
      error = `${tr("Lista condivisa", "Shared list")}: ${e}`;
    }
  }

  /** "adesso", "5 min fa", "alle 18:20", "il 14/11". */
  function ago(unix: number): string {
    const secs = Math.floor(Date.now() / 1000) - unix;
    if (secs < 60) return tr("adesso", "just now");
    if (secs < 3600) return `${Math.floor(secs / 60)} ${tr("min fa", "min ago")}`;
    const d = new Date(unix * 1000);
    if (d.toDateString() === new Date().toDateString()) return `${tr("alle", "at")} ${clock(unix)}`;
    return `${tr("il", "on")} ${d.toLocaleDateString(locale(), { day: "numeric", month: "numeric" })}`;
  }

  async function chooseBackupFolder() {
    const dir = await openDialog({ directory: true, title: tr("Cartella dei backup (es. su D: o in OneDrive)", "Backup folder (e.g. on D: or in OneDrive)") });
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
      backupResult = tr("Note ripristinate. Lo stato di prima è in un nuovo backup.", "Notes restored. The previous state is in a new backup.");
      await loadBackups();
    } catch (e) {
      error = `${tr("Ripristino", "Restore")}: ${e}`;
    } finally {
      restoring = false;
    }
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
    refreshSync();
    invoke<boolean>("is_store_build").then((v) => (storeBuild = v));
    invoke<CloudFolder[]>("sync_cloud_folders").then((list) => (clouds = list));
    invoke<FolderInfo[]>("list_folders").then((list) => (folders = list));
    dialog?.focus();
    // Cambiato dal menu della tray mentre il pannello è aperto.
    const unlisten = listen("autostart-changed", async () => (autostart = await invoke<boolean>("get_autostart")));
    const unlistenSync = listen<SyncStatus>("sync-status-changed", ({ payload }) => (syncStatus = payload));
    return () => {
      unlistenSync.then((fn) => fn());
      unlisten.then((fn) => fn());
    };
  });
</script>

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={tr("Impostazioni", "Settings")}
    tabindex="-1"
    bind:this={dialog}
    onkeydown={(e) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onclose();
      }
    }}
  >
    <nav aria-label={tr("Sezioni delle impostazioni", "Settings sections")}>
      <h2>{tr("Impostazioni", "Settings")}</h2>
      {#each TABS as t (t.id)}
        <button class="tab" class:on={tab === t.id} aria-current={tab === t.id ? "page" : undefined} onclick={() => (tab = t.id)}>
          <span class="tab-icon" aria-hidden="true">{t.icon}</span>
          {t.label}
          {#if t.id === "reminders" && quietUntil}<span class="tab-dot" title={tr("Notifiche in pausa", "Notifications paused")}></span>{/if}
          {#if t.id === "sync" && settings.sync_dir && syncStatus.last_error}<span class="tab-dot warn" title={tr("Problema di sincronizzazione", "Sync problem")}></span>{/if}
          {#if t.id === "about" && update_}<span class="tab-dot" title={tr("Aggiornamento disponibile", "Update available")}></span>{/if}
        </button>
      {/each}
    </nav>

    <section class="content">
      <header>
        <div>
          <h3>{current.label}</h3>
          <p class="intro">{current.intro}</p>
        </div>
        <button class="close" onclick={onclose} aria-label={tr("Chiudi le impostazioni", "Close settings")}>✕</button>
      </header>

      {#if error}<p class="error">{error}</p>{/if}

      {#if tab === "general"}
        <div class="group">
          {#if storeBuild}
            <div class="row">
              <span>
                {tr("Avvia Tabby con Windows", "Start Tabby with Windows")}
                <small class="desc">{tr("Si accende nelle impostazioni di Windows, in App → Avvio.", "Turn it on in Windows Settings, under Apps → Startup.")}</small>
              </span>
              <button class="plain" onclick={() => invoke("open_startup_settings")}>{tr("Apri…", "Open…")}</button>
            </div>
          {:else}
            <label class="row check">
              <span>{tr("Avvia Tabby con Windows", "Start Tabby with Windows")}</span>
              <input type="checkbox" checked={autostart} onchange={toggleAutostart} />
            </label>
          {/if}
          <label class="row">
            <span>
              {tr("Lingua", "Language")}
              <small class="desc">{tr("Il menu dell'icona vicino all'orologio cambia al prossimo avvio.", "The menu of the icon near the clock changes at the next start.")}</small>
            </span>
            <select value={settings.language} onchange={(e) => update({ language: e.currentTarget.value as Settings["language"] })}>
              <option value="auto">{tr("Automatica (come Windows)", "Automatic (like Windows)")}</option>
              <option value="it">Italiano</option>
              <option value="en">English</option>
            </select>
          </label>
          <div class="row">
            <span>
              {tr("Tema della finestra «Tutte le note»", "Theme of the “All notes” window")}
              <small class="desc">{tr("Automatico segue il tema di Windows.", "Automatic follows the Windows theme.")}</small>
            </span>
            <div class="segmented">
              <button class:on={settings.theme === "auto"} onclick={() => update({ theme: "auto" })}>{tr("Automatico", "Automatic")}</button>
              <button class:on={settings.theme === "light"} onclick={() => update({ theme: "light" })}>{tr("Chiaro", "Light")}</button>
              <button class:on={settings.theme === "dark"} onclick={() => update({ theme: "dark" })}>{tr("Scuro", "Dark")}</button>
            </div>
          </div>
        </div>

        <h4>{tr("Deck sul bordo dello schermo", "Deck on the screen edge")}</h4>
        <div class="group">
          <div class="row">
            <span>{tr("Bordo", "Edge")}</span>
            <div class="segmented">
              <button class:on={settings.side === "left"} onclick={() => update({ side: "left" })}>{tr("Sinistra", "Left")}</button>
              <button class:on={settings.side === "right"} onclick={() => update({ side: "right" })}>{tr("Destra", "Right")}</button>
            </div>
          </div>
          {#if monitors.length > 1}
            <label class="row">
              <span>{tr("Schermo", "Screen")}</span>
              <select value={settings.monitor ?? ""} onchange={(e) => update({ monitor: e.currentTarget.value || null })}>
                <option value="">{tr("Principale", "Main")}</option>
                {#each monitors as m (m.name)}
                  <option value={m.name}>{m.label}</option>
                {/each}
              </select>
            </label>
          {/if}
          <label class="row">
            <span>{tr("Larghezza della nota aperta", "Width of the open note")} <small class="value">{settings.note_width} px</small></span>
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
            <span>
              {tr("Trasparenza delle linguette", "Tab transparency")} <small class="value">{settings.deck_transparency}%</small>
              <small class="desc">{tr("Più alta, più si vede lo sfondo attraverso.", "The higher, the more the background shows through.")}</small>
            </span>
            <input
              type="range"
              min="0"
              max="100"
              step="5"
              value={settings.deck_transparency}
              onchange={(e) => update({ deck_transparency: Number(e.currentTarget.value) })}
            />
          </label>
        </div>
      {:else if tab === "notes"}
        <h4>{tr("Testo", "Text")}</h4>
        <div class="group">
          <label class="row">
            <span>{tr("Carattere", "Font")}</span>
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
            <span>{tr("Dimensione", "Size")} <small class="value">{settings.editor_size} px</small></span>
            <input
              type="range"
              min="13"
              max="24"
              value={settings.editor_size}
              onchange={(e) => update({ editor_size: Number(e.currentTarget.value) })}
            />
          </label>
          <p class="sample" style:font-family="var(--editor-font)" style:font-size="{settings.editor_size}px">
            {tr("Comprare il latte e chiamare Marta", "Buy milk and call Martha")}
          </p>
        </div>

        <h4>{tr("Colore delle note nuove", "Colour of new notes")}</h4>
        <div class="group">
          <div class="colors">
            <button
              class="cycle"
              class:on={settings.new_note_color === "cycle"}
              onclick={() => update({ new_note_color: "cycle" })}
            >{tr("A rotazione", "Rotating")}</button>
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
          <small class="desc pad">
            {settings.new_note_color === "cycle"
              ? tr("Ogni nota nuova prende il colore successivo.", "Each new note takes the next colour.")
              : `${tr("Le note nuove saranno", "New notes will be")} ${COLOR_NAMES[settings.new_note_color]?.toLowerCase() ?? ""}. ${tr("Puoi sempre cambiarlo dalla nota.", "You can always change it from the note.")}`}
          </small>
        </div>

        <h4>{tr("🔒 Protezione con PIN", "🔒 PIN protection")}</h4>
        <div class="group">
          {#if !hasPin()}
            <small class="desc pad">
              {tr("Nessun PIN impostato. Attiva \"Proteggi con PIN\" su una nota per impostarne uno.", "No PIN set. Turn on \"Protect with PIN\" on a note to set one.")}
            </small>
          {:else if changingPin}
            <label class="row">
              <span>{tr("PIN attuale", "Current PIN")}</span>
              <input type="password" inputmode="numeric" maxlength="6" bind:value={oldPin} />
            </label>
            <label class="row">
              <span>{tr("Nuovo PIN", "New PIN")}</span>
              <input type="password" inputmode="numeric" maxlength="6" bind:value={newPin} />
            </label>
            <label class="row">
              <span>{tr("Conferma nuovo PIN", "Confirm new PIN")}</span>
              <input type="password" inputmode="numeric" maxlength="6" bind:value={newPinConfirm} />
            </label>
            <div class="row">
              <button class="plain" onclick={submitChangePin}>{tr("Cambia PIN", "Change PIN")}</button>
              <button
                class="plain"
                onclick={() => {
                  changingPin = false;
                  oldPin = newPin = newPinConfirm = "";
                }}
              >
                {tr("Annulla", "Cancel")}
              </button>
            </div>
          {:else}
            <div class="row">
              <span>{tr("Le note protette richiedono questo PIN per essere lette o modificate.", "Protected notes need this PIN to be read or edited.")}</span>
              <button class="plain" onclick={() => (changingPin = true)}>{tr("Cambia PIN", "Change PIN")}</button>
            </div>
            <div class="row">
              <span>{tr("Hai dimenticato il PIN? Il contenuto non si perde, ma tutte le note perdono la protezione.", "Forgot the PIN? The content is not lost, but all notes lose their protection.")}</span>
              <button class:danger={confirmResetPin} onclick={resetPin}>
                {confirmResetPin ? tr("Conferma rimozione", "Confirm removal") : tr("Rimuovi protezione da tutte le note", "Remove protection from all notes")}
              </button>
            </div>
          {/if}
        </div>
      {:else if tab === "reminders"}
        <h4>{tr("Notifiche", "Notifications")}</h4>
        <div class="group">
          <label class="row check">
            <span>{tr("Suono", "Sound")}</span>
            <input type="checkbox" checked={settings.sound} onchange={(e) => update({ sound: e.currentTarget.checked })} />
          </label>
          {#if settings.sound}
            <div class="row sub">
              <span>{tr("Quale suono", "Which sound")}</span>
              <div class="buttons">
                <select value={settings.sound_name} onchange={(e) => update({ sound_name: e.currentTarget.value })}>
                  {#each SOUNDS as [key, label] (key)}
                    <option value={key}>{label}</option>
                  {/each}
                </select>
                <button class="plain" onclick={() => invoke("preview_sound", { name: settings.sound_name })}>{tr("▶ Prova", "▶ Try")}</button>
              </div>
            </div>
          {/if}
          <label class="row">
            <span>
              {tr("Ripeti la notifica", "Repeat the notification")}
              <small class="desc">{tr("Finché non la segni come fatta o la posticipi.", "Until you mark it as done or snooze it.")}</small>
            </span>
            <select value={settings.repeat_minutes} onchange={(e) => update({ repeat_minutes: Number(e.currentTarget.value) })}>
              <option value={0}>{tr("Mai", "Never")}</option>
              {#each [1, 2, 5, 10, 15, 30] as m (m)}
                <option value={m}>{tr("Ogni", "Every")} {m} min</option>
              {/each}
            </select>
          </label>
          <label class="row">
            <span>
              {tr("Ora di «Domani»", "Time of “Tomorrow”")}
              <small class="desc">{tr("Quando posticipi un promemoria a domani.", "When you snooze a reminder until tomorrow.")}</small>
            </span>
            <input
              type="time"
              value={settings.tomorrow_time}
              onchange={(e) => e.currentTarget.value && update({ tomorrow_time: e.currentTarget.value })}
            />
          </label>
        </div>

        <h4>{tr("Non disturbare", "Do not disturb")}</h4>
        <div class="group">
          <div class="row">
            <span>
              {#if quietUntil}
                <strong>🌙 {tr("In pausa fino alle", "Paused until")} {clock(quietUntil)}</strong>
                <small class="desc">{tr("I promemoria arrivano tutti insieme alla fine.", "Reminders all arrive together at the end.")}</small>
              {:else}
                {tr("Metti in pausa le notifiche", "Pause notifications")}
              {/if}
            </span>
            <div class="buttons">
              {#if settings.dnd_until}
                <button class="plain" onclick={() => setDnd(null)}>{tr("Riattiva", "Turn back on")}</button>
              {:else}
                <button class="plain" onclick={() => setDnd(60)}>{tr("1 ora", "1 hour")}</button>
                <button class="plain" onclick={() => setDnd("morning")}>{tr("Fino a domattina", "Until tomorrow morning")}</button>
              {/if}
            </div>
          </div>
          <label class="row check">
            <span>
              {tr("Ogni giorno in una fascia oraria", "Every day at set hours")}
              <small class="desc">{tr("Per esempio di notte.", "For example at night.")}</small>
            </span>
            <input type="checkbox" checked={settings.quiet_enabled} onchange={(e) => update({ quiet_enabled: e.currentTarget.checked })} />
          </label>
          {#if settings.quiet_enabled}
            <div class="row sub">
              <span>{tr("Dalle … alle …", "From … to …")}</span>
              <div class="buttons">
                <input
                  type="time"
                  aria-label={tr("Inizio", "Start")}
                  value={settings.quiet_from}
                  onchange={(e) => e.currentTarget.value && update({ quiet_from: e.currentTarget.value })}
                />
                <span>→</span>
                <input
                  type="time"
                  aria-label={tr("Fine", "End")}
                  value={settings.quiet_to}
                  onchange={(e) => e.currentTarget.value && update({ quiet_to: e.currentTarget.value })}
                />
              </div>
            </div>
          {/if}
        </div>

        <h4>{tr("📋 Note dimenticate", "📋 Forgotten notes")}</h4>
        <div class="group">
          <label class="row check">
            <span>
              {tr("Avvisami delle note ferme da tempo", "Tell me about notes left untouched")}
              <small class="desc">{tr("Note attive, non fissate, senza promemoria o timer, non toccate da un po'.", "Active notes, not pinned, without reminder or timer, untouched for a while.")}</small>
            </span>
            <input
              type="checkbox"
              checked={settings.forgotten_enabled}
              onchange={(e) => update({ forgotten_enabled: e.currentTarget.checked })}
            />
          </label>
          {#if settings.forgotten_enabled}
            <div class="row sub">
              <span>{tr("Dopo quanti giorni", "After how many days")}</span>
              <input
                type="number"
                min="1"
                max="365"
                value={settings.forgotten_days}
                onchange={(e) => update({ forgotten_days: Number(e.currentTarget.value) })}
              />
            </div>
          {/if}
        </div>
      {:else if tab === "sync"}
        <h4>{tr("I tuoi PC", "Your PCs")}</h4>
        {#if !settings.sync_dir}
          <div class="group">
            <p class="desc pad">
              {tr("Tabby non ha un server: per avere le stesse note su più PC usa un servizio cloud che hai già, come Google Drive, OneDrive o Dropbox. Serve la sua app installata e collegata al tuo account su ogni PC: Tabby scrive le note nella sua cartella e l'app le porta sugli altri PC.", "Tabby has no server: to have the same notes on several PCs it uses a cloud service you already have, such as Google Drive, OneDrive or Dropbox. Its app must be installed and signed in on every PC: Tabby writes the notes in its folder and the app carries them to the other PCs.")}
            </p>
            {#if clouds.length > 0}
              {#each clouds as c (c.path)}
                <div class="row">
                  <span class="path">
                    <strong>{c.service}</strong>
                    <small class="desc" title={c.path}>{c.path}</small>
                  </span>
                  <button class="plain primary" onclick={() => useCloud(c.path)}>{tr("Usa questa", "Use this")}</button>
                </div>
              {/each}
            {:else}
              <p class="desc pad warn">{tr("Su questo PC non ho trovato Google Drive, OneDrive o Dropbox.", "I didn't find Google Drive, OneDrive or Dropbox on this PC.")}</p>
            {/if}
            <div class="row">
              <span>{tr("Un'altra cartella (anche di rete)", "Another folder (network ones too)")}</span>
              <button class="plain" onclick={chooseSyncFolder}>{tr("Scegli cartella…", "Choose folder…")}</button>
            </div>
          </div>
          <details class="howto">
            <summary>{tr("Come si fa, passo per passo", "How to do it, step by step")}</summary>
            <ol>
              <li>
                {tr("Installa l'app del servizio che usi e accedi con il tuo account:", "Install the app of the service you use and sign in with your account:")}
                {#each DOWNLOADS as [name, url] (name)}
                  <button class="link" onclick={() => openUrl(url)}>{name}</button>
                {/each}
                {tr("(OneDrive è già in Windows).", "(OneDrive is already in Windows).")}
              </li>
              <li>{tr("Torna qui: la cartella compare nell'elenco sopra. Premi «Usa questa».", "Come back here: the folder appears in the list above. Press “Use this”.")}</li>
              <li>{tr("Sugli altri PC: stessa app, stesso account, e qui la stessa scelta. Le note si uniscono da sole, senza perderne nessuna.", "On your other PCs: same app, same account, and the same choice here. The notes merge by themselves, without losing any.")}</li>
            </ol>
          </details>
          <p class="tip">
            {tr("💡 Viaggiano note, cartelle, promemoria, colori, note fissate, cestino e PIN. Restano di ogni PC: posizione del deck, suoni, scorciatoie, Non disturbare e backup.", "💡 Notes, folders, reminders, colours, pinned notes, trash and PIN travel. Each PC keeps its own deck position, sounds, shortcuts, Do not disturb and backups.")}
          </p>
        {:else}
          <div class="group">
            <div class="row">
              <span class="path">
                <strong>
                  {syncStatus.needs_passphrase ? tr("🔑 Serve la frase segreta", "🔑 Passphrase needed") : syncStatus.last_error ? tr("⚠ Sincronizzazione ferma", "⚠ Sync stopped") : tr("✓ Sincronizzato", "✓ Synced")}
                </strong>
                <small class="desc" title={settings.sync_dir}>
                  {syncStatus.service ? `${syncStatus.service} · ` : ""}{settings.sync_dir}
                </small>
                {#if !syncStatus.service && !syncStatus.last_error}
                  <small class="desc warn">{tr("Questa cartella non sembra di Google Drive, OneDrive o Dropbox: se non è una cartella di rete, le note restano solo su questo PC.", "This folder doesn't look like Google Drive, OneDrive or Dropbox: unless it is a network folder, the notes stay on this PC only.")}</small>
                {/if}
                {#if syncStatus.last_error && !syncStatus.needs_passphrase}
                  <small class="desc warn">{syncStatus.last_error}</small>
                {:else if syncStatus.last_sync && !syncStatus.needs_passphrase}
                  <small class="desc">{tr("Ultimo controllo", "Last check")} {ago(syncStatus.last_sync)} · {tr("le modifiche arrivano in pochi secondi", "changes arrive within seconds")}</small>
                {/if}
              </span>
              <div class="buttons">
                <button class="plain" onclick={() => invoke("sync_now")}>{tr("↻ Ora", "↻ Now")}</button>
                <button class="plain" onclick={chooseSyncFolder}>{tr("Cambia…", "Change…")}</button>
              </div>
            </div>
            {#each syncStatus.devices as d (d.name + d.this_pc)}
              <div class="row">
                <span>💻 {d.name}{#if d.this_pc} <small class="value">{tr("(questo PC)", "(this PC)")}</small>{/if}</span>
                <small class="value">{tr("ultima modifica", "last change")} {ago(d.seen_at)}</small>
              </div>
            {/each}
          </div>

          <h4>{tr("Cifratura", "Encryption")}</h4>
          <div class="group">
            {#if syncStatus.needs_passphrase || passphraseOpen}
              <p class="desc pad">
                {#if syncStatus.needs_passphrase}
                  {tr("La cartella è cifrata da un altro PC: scrivi la stessa frase segreta per leggere e scrivere le note.", "The folder was encrypted by another PC: type the same passphrase to read and write the notes.")}
                {:else}
                  {tr("Scegli una frase segreta (almeno 8 caratteri) e scrivila uguale su tutti i PC. Se la dimentichi, le note nella cartella condivisa non si possono più leggere (quelle su ogni PC restano).", "Choose a passphrase (at least 8 characters) and type it the same on all your PCs. If you forget it, the notes in the shared folder can no longer be read (the ones on each PC stay).")}
                {/if}
              </p>
              <form class="row" onsubmit={(e) => { e.preventDefault(); savePassphrase(); }}>
                <input
                  class="passphrase"
                  type="password"
                  autocomplete="new-password"
                  placeholder={tr("Frase segreta", "Passphrase")}
                  bind:value={passphrase}
                />
                <div class="buttons">
                  {#if passphraseOpen && !syncStatus.needs_passphrase}
                    <button type="button" class="plain" onclick={() => { passphraseOpen = false; passphrase = ""; }}>{tr("Annulla", "Cancel")}</button>
                  {/if}
                  <button class="plain primary" disabled={passphraseBusy || passphrase.trim().length < 8}>
                    {passphraseBusy ? tr("Attendere…", "Please wait…") : syncStatus.needs_passphrase ? tr("Sblocca", "Unlock") : tr("Cifra la cartella", "Encrypt the folder")}
                  </button>
                </div>
              </form>
            {:else if syncStatus.encrypted}
              <div class="row">
                <span>
                  {tr("🔒 Cifrata con la frase segreta", "🔒 Encrypted with the passphrase")}
                  <small class="desc">{tr("Su Drive ci sono solo dati illeggibili, anche per Google. Le copie per il telefono sono spente.", "Only unreadable data is on Drive, even for Google. Phone copies are off.")}</small>
                </span>
                <button class="plain" class:danger={confirmDecrypt} onclick={removeEncryption}>
                  {confirmDecrypt ? tr("Conferma", "Confirm") : tr("Togli", "Remove")}
                </button>
              </div>
            {:else}
              <div class="row">
                <span>
                  {tr("Non cifrata", "Not encrypted")}
                  <small class="desc">{tr("Le note nella cartella condivisa sono leggibili da chi può aprirla (anche quelle col PIN).", "Anyone who can open the shared folder can read the notes in it (PIN-protected ones too).")}</small>
                </span>
                <button class="plain" onclick={() => (passphraseOpen = true)}>{tr("Cifra…", "Encrypt…")}</button>
              </div>
            {/if}
          </div>

          {#if !syncStatus.encrypted}
            <h4>{tr("Sul telefono", "On your phone")}</h4>
            <div class="group">
              <label class="row check">
                <span>
                  {tr("Copie da leggere sul telefono", "Copies to read on your phone")}
                  <small class="desc">
                    {tr("Un file per nota nella cartella «Note da leggere», da aprire con l'app di Google Drive, OneDrive o Dropbox. Solo lettura; le note col PIN, archiviate o nel cestino non vengono copiate.", "One file per note in the “Notes to read” folder, to open with the Google Drive, OneDrive or Dropbox app. Read-only; PIN-protected, archived or trashed notes are not copied.")}
                  </small>
                </span>
                <input type="checkbox" checked={syncStatus.phone_copy} onchange={(e) => setPhoneCopy(e.currentTarget.checked)} />
              </label>
            </div>
          {/if}

          <div class="group">
            <div class="row">
              <span>
                {tr("Smetti di sincronizzare", "Stop syncing")}
                <small class="desc">{tr("Le note restano su questo PC; la cartella condivisa e gli altri PC non vengono toccati.", "The notes stay on this PC; the shared folder and the other PCs are not touched.")}</small>
              </span>
              <button class="plain" class:danger={syncConfirmOff} onclick={syncDisconnect}>
                {syncConfirmOff ? tr("Conferma", "Confirm") : tr("Scollega", "Disconnect")}
              </button>
            </div>
          </div>
          <p class="tip">
            {tr("💡 Se la stessa nota cambia su due PC mentre uno è spento, vince la modifica più recente e l'altra resta nella cronologia della nota. Il PIN impostato per ultimo vale su tutti i PC.", "💡 If the same note changes on two PCs while one is off, the most recent change wins and the other stays in the note's history. The PIN set last applies on all PCs.")}
          </p>
        {/if}

        <h4>{tr("Liste condivise con altre persone", "Lists shared with other people")}</h4>
        <div class="group">
          <p class="desc pad">
            {tr("Una cartella di note (per esempio la spesa) che vedete e modificate in più persone. Si usa una cartella di Drive, OneDrive o Dropbox condivisa con loro. Le note col PIN non vengono mai condivise.", "A folder of notes (for example the shopping) that several people see and edit. It uses a Drive, OneDrive or Dropbox folder shared with them. PIN-protected notes are never shared.")}
          </p>
          {#each syncStatus.shares as share (share.folder_uid)}
            <div class="row">
              <span class="path">
                <strong>👥 {share.name}</strong>
                <small class="desc" title={share.dir}>{share.dir}</small>
                {#if share.last_error}
                  <small class="desc warn">{share.last_error}</small>
                {:else}
                  <small class="desc">
                    {share.devices.filter((d) => !d.this_pc).length > 0
                      ? `${tr("Con", "With")}: ${share.devices.filter((d) => !d.this_pc).map((d) => d.name).join(", ")}`
                      : tr("Nessun altro ancora collegato", "Nobody else connected yet")}
                  </small>
                {/if}
              </span>
              <button class="plain" class:danger={confirmLeave === share.folder_uid} onclick={() => leaveShare(share.folder_uid)}>
                {confirmLeave === share.folder_uid ? tr("Conferma", "Confirm") : tr("Smetti", "Stop")}
              </button>
            </div>
          {/each}
          <div class="row">
            <select bind:value={shareFolderId} aria-label={tr("Cartella da condividere", "Folder to share")}>
              <option value={null}>{tr("Condividi una cartella…", "Share a folder…")}</option>
              {#each folders.filter((f) => !syncStatus.shares.some((x) => x.name === f.name)) as f (f.id)}
                <option value={f.id}>{f.name}</option>
              {/each}
            </select>
            <div class="buttons">
              <button class="plain" disabled={shareFolderId === null} onclick={shareFolder}>{tr("Scegli dove…", "Choose where…")}</button>
              <button class="plain" onclick={joinShare}>{tr("Unisciti…", "Join…")}</button>
            </div>
          </div>
        </div>
        <p class="tip">
          {tr("💡 Per condividere: scegli la cartella di note, poi una cartella di Drive vuota, e da Google Drive condividi quella cartella con le altre persone. Loro, in Tabby, premono «Unisciti…» e scelgono la cartella che hanno ricevuto.", "💡 To share: choose the note folder, then an empty Drive folder, and share that folder with the other people from Google Drive. In Tabby they press “Join…” and choose the folder they received.")}
        </p>
      {:else if tab === "backup"}
        <h4>{tr("Backup automatico", "Automatic backup")}</h4>
        <div class="group">
          <label class="row check">
            <span>
              {tr("Ogni giorno", "Every day")}
              <small class="desc">{tr("Copia di note e immagini.", "Copy of notes and images.")}</small>
            </span>
            <input type="checkbox" checked={settings.backup_enabled} onchange={(e) => update({ backup_enabled: e.currentTarget.checked })} />
          </label>
          <div class="row">
            <span class="path">
              {tr("Cartella", "Folder")}
              <small class="desc" title={backupFolder}>{settings.backup_dir ? backupFolder : tr("Predefinita (nella cartella dei dati)", "Default (in the data folder)")}</small>
            </span>
            <div class="buttons">
              {#if settings.backup_dir}
                <button class="plain" onclick={defaultBackupFolder}>{tr("Predefinita", "Default")}</button>
              {/if}
              <button class="plain" onclick={chooseBackupFolder}>{tr("Cambia…", "Change…")}</button>
            </div>
          </div>
          {#if !settings.backup_dir}
            <p class="tip">{tr("💡 Meglio un altro disco o una cartella di OneDrive: se si guasta questo disco, i backup restano al sicuro.", "💡 Better another disk or a OneDrive folder: if this disk breaks, the backups stay safe.")}</p>
          {/if}
          <label class="row">
            <span>{tr("Quanti tenerne", "How many to keep")}</span>
            <select value={settings.backup_keep} onchange={(e) => update({ backup_keep: Number(e.currentTarget.value) })}>
              {#each [3, 7, 14, 30] as n (n)}
                <option value={n}>{tr("Gli ultimi", "The last")} {n}</option>
              {/each}
            </select>
          </label>
          <div class="row">
            <span>
              {backupResult || (backups[0] ? `${tr("Ultimo backup", "Last backup")}: ${backupDate(backups[0].created_at)}` : tr("Nessun backup ancora", "No backup yet"))}
            </span>
            <div class="buttons">
              <button class="plain" onclick={() => invoke("open_backup_folder")}>{tr("Apri", "Open")}</button>
              <button class="plain" disabled={backingUp || restoring} onclick={backupNow}>
                {backingUp ? "Backup…" : tr("Esegui ora", "Run now")}
              </button>
            </div>
          </div>
        </div>

        {#if backups.length > 0}
          <h4>{tr("Ripristina", "Restore")}</h4>
          <div class="group">
            <p class="desc pad">
              {tr("Le note attuali vengono sostituite da quelle del backup. Prima Tabby salva lo stato attuale in un nuovo backup, così puoi sempre tornare indietro.", "The current notes are replaced by those in the backup. First Tabby saves the current state in a new backup, so you can always go back.")}
            </p>
            <details class="backups">
              <summary>{tr("Scegli un backup", "Choose a backup")} ({backups.length})</summary>
              {#each backups as b (b.name)}
                <div class="backup-row">
                  <span>{backupDate(b.created_at)} <small class="value">{plural(b.notes, ["nota", "note"], ["note", "notes"])} · {plural(b.media_files, ["file", "file"], ["file", "files"])}</small></span>
                  <button
                    class="plain"
                    class:danger={confirmRestore === b.name}
                    disabled={restoring || backingUp}
                    onclick={() => restore(b.name)}
                  >
                    {restoring && confirmRestore === null ? "…" : confirmRestore === b.name ? tr("Conferma ripristino", "Confirm restore") : tr("Ripristina", "Restore")}
                  </button>
                </div>
              {/each}
            </details>
          </div>
        {/if}

        <h4>{tr("Dati", "Data")}</h4>
        <div class="group">
          <div class="row">
            <span>{tr("Note, immagini e impostazioni sono salvate su questo PC.", "Notes, images and settings are stored on this PC.")}</span>
            <button class="plain" onclick={() => invoke("open_data_folder")}>{tr("Apri la cartella", "Open the folder")}</button>
          </div>
        </div>
      {:else}
        <div class="group">
          <p class="about">
            <strong>Tabby</strong>{version ? ` ${version}` : ""}<br />
            {tr("Creato da", "Made by")} <strong>Roberto Pisco Pisconti</strong>
          </p>
          <div class="row">
            <span>
              {#if update_}
                <strong>{tr("Nuova versione", "New version")} {update_.version} {tr("disponibile", "available")}</strong>
              {:else if updateStatus === "checking"}
                {tr("Controllo aggiornamenti…", "Checking for updates…")}
              {:else if updateStatus === "latest"}
                {tr("Hai l'ultima versione.", "You have the latest version.")}
              {:else if updateStatus === "store"}
                {tr("Gli aggiornamenti arrivano dal Microsoft Store.", "Updates come from the Microsoft Store.")}
              {:else if updateStatus === "failed"}
                {tr("Non riesco a controllare gli aggiornamenti.", "I can't check for updates.")}
              {:else}
                {tr("Aggiornamenti", "Updates")}
              {/if}
            </span>
            {#if updateStatus === "store"}
              <span></span>
            {:else if update_}
              <button class="plain primary" onclick={() => update_ && openUrl(update_.url)}>{tr("Scarica", "Download")}</button>
            {:else}
              <button class="plain" disabled={updateStatus === "checking"} onclick={checkUpdate}>{tr("Controlla ora", "Check now")}</button>
            {/if}
          </div>
        </div>

        <h4>{tr("Scorciatoie da tastiera", "Keyboard shortcuts")}</h4>
        <div class="group">
          <dl>
            {#each SHORTCUTS as [keys, action] (keys)}
              <dt><kbd>{keys}</kbd></dt>
              <dd>{action}</dd>
            {/each}
          </dl>
          <p class="desc pad">{tr("Se un'altra app usa già una di queste combinazioni, quella scorciatoia non è disponibile.", "If another app already uses one of these combinations, that shortcut is not available.")}</p>
        </div>
      {/if}
    </section>
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
    width: min(760px, calc(100vw - 40px));
    height: min(600px, calc(100vh - 60px));
    display: flex;
    overflow: hidden;
    box-sizing: border-box;
    border-radius: 16px;
    background: var(--pane);
    color: var(--text);
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
    outline: none;
  }

  /* Colonna delle sezioni */
  nav {
    flex: none;
    width: 190px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 18px 10px;
    border-right: 1px solid var(--line);
    background: color-mix(in srgb, var(--hover) 55%, var(--pane));
  }
  nav h2 {
    margin: 0 10px 12px;
    font-size: 17px;
  }
  .tab {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    color: var(--muted);
    font-weight: 600;
    text-align: left;
  }
  .tab:hover {
    background: var(--hover);
    color: var(--text);
  }
  .tab.on {
    background: var(--pane);
    color: var(--text);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.12);
  }
  .tab-icon {
    width: 20px;
    text-align: center;
  }
  .tab-dot {
    width: 7px;
    height: 7px;
    margin-left: auto;
    border-radius: 50%;
    background: var(--focus);
  }

  .tab-dot.warn {
    background: var(--danger, #e55);
  }

  /* Contenuto della sezione */
  .content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    padding: 18px 22px 24px;
  }
  .content::-webkit-scrollbar {
    width: 8px;
  }
  .content::-webkit-scrollbar-button {
    display: none;
  }
  .content::-webkit-scrollbar-thumb {
    border: 2px solid var(--pane);
    border-radius: 4px;
    background: var(--line);
  }
  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    margin-bottom: 6px;
  }
  h3 {
    margin: 2px 0 2px;
    font-size: 17px;
  }
  .intro {
    margin: 0;
    color: var(--muted);
    font-size: 13px;
  }
  h4 {
    margin: 18px 0 6px;
    color: var(--muted);
    font-size: 11px;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .close {
    flex: none;
    width: 30px;
    height: 30px;
    border: 0;
    border-radius: 8px;
    background: transparent;
  }
  .close:hover {
    background: var(--hover);
  }

  /* Gruppi di impostazioni: riquadri con righe separate da linee sottili. */
  .group {
    margin-top: 12px;
    padding: 2px 14px;
    border: 1px solid var(--line);
    border-radius: 12px;
  }
  h4 + .group {
    margin-top: 0;
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    min-height: 40px;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
  }
  .group > :last-child,
  .group > .row:last-child {
    border-bottom: 0;
  }
  .row > span {
    flex: 1;
    min-width: 0;
  }
  .row.sub {
    padding-left: 14px;
  }
  .check {
    cursor: pointer;
  }
  .desc {
    display: block;
    margin: 2px 0 0;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.4;
  }
  .passphrase {
    flex: 1;
    min-width: 0;
    padding: 6px 10px;
    border: 1px solid var(--line, rgba(0, 0, 0, 0.15));
    border-radius: 6px;
    background: transparent;
    color: inherit;
    font: inherit;
  }
  .howto {
    margin: 4px 2px 14px;
    font-size: 13px;
  }
  .howto summary {
    cursor: pointer;
    font-weight: 600;
  }
  .howto ol {
    margin: 8px 0 0;
    padding-left: 20px;
    line-height: 1.6;
  }
  .link {
    border: 0;
    padding: 0 2px;
    background: transparent;
    color: var(--focus, #2563eb);
    font: inherit;
    text-decoration: underline;
    cursor: pointer;
  }
  .desc.warn {
    color: var(--danger, #e55);
  }
  .desc.pad {
    margin: 0;
    padding: 8px 0;
  }
  .value {
    margin-left: 6px;
    color: var(--muted);
    font-size: 12px;
    font-variant-numeric: tabular-nums;
  }
  .tip {
    margin: 0;
    padding: 8px 0;
    border-bottom: 1px solid var(--line);
    color: var(--muted);
    font-size: 12px;
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
    width: 170px;
    accent-color: var(--focus);
  }
  input[type="checkbox"] {
    flex: none;
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
  .path .desc {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sample {
    margin: 0;
    padding: 10px 0 12px;
    color: var(--muted);
  }
  .backups summary {
    padding: 8px 0;
    cursor: pointer;
  }
  .backup-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 0;
  }
  .plain {
    padding: 5px 12px;
    border: 1px solid var(--line);
    border-radius: 8px;
    background: var(--pane);
    white-space: nowrap;
  }
  .plain:hover {
    background: var(--hover);
  }
  .plain:disabled {
    opacity: 0.55;
    cursor: default;
  }
  .plain.primary {
    border-color: var(--focus);
    background: var(--focus);
    color: #fff;
  }
  .danger {
    color: var(--danger, #e55);
    border-color: color-mix(in srgb, var(--danger, #e55) 40%, transparent);
  }
  .danger:hover {
    background: color-mix(in srgb, var(--danger, #e55) 10%, transparent);
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
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    padding: 12px 0 4px;
  }
  .cycle {
    padding: 3px 10px;
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
    width: 22px;
    height: 22px;
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
    gap: 8px 14px;
    margin: 0;
    padding: 12px 0 4px;
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
  .about {
    margin: 0;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
    line-height: 1.5;
  }
  .error {
    margin: 10px 0 0;
    padding: 8px 10px;
    border-radius: 8px;
    background: color-mix(in srgb, var(--danger) 15%, transparent);
    color: var(--danger);
  }

  /* Finestra stretta: le sezioni passano in alto, in riga. */
  @media (max-width: 620px) {
    .dialog {
      flex-direction: column;
    }
    nav {
      width: auto;
      flex-direction: row;
      overflow-x: auto;
      padding: 10px;
      border-right: 0;
      border-bottom: 1px solid var(--line);
    }
    nav h2 {
      display: none;
    }
    .tab {
      flex: none;
    }
  }
</style>
