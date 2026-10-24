<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { hasPin, settings } from "$lib/settings.svelte";
  import type { Note } from "$lib/notes";

  let {
    note,
    unlocked,
    onunlocked,
    onlockedchange,
  }: {
    note: Note;
    /** Contenuto reale già recuperato con `unlock_note` per questa apertura, o `null`. */
    unlocked: Note | null;
    onunlocked: (note: Note) => void;
    onlockedchange: (locked: boolean) => void;
  } = $props();

  let settingPin = $state(false);
  let newPin = $state("");
  let newPinConfirm = $state("");
  let error = $state("");
  let root = $state<HTMLElement>();

  async function toggle() {
    error = "";
    if (note.locked) {
      // Si può togliere la protezione solo dopo aver già verificato il PIN in questa apertura.
      if (!unlocked) return;
      await invoke("set_note_locked", { id: note.id, locked: false });
      onlockedchange(false);
      return;
    }
    if (!hasPin()) {
      settingPin = true;
      return;
    }
    await invoke("set_note_locked", { id: note.id, locked: true });
    onlockedchange(true);
  }

  async function confirmNewPin() {
    error = "";
    if (newPin !== newPinConfirm) {
      error = "I due PIN non coincidono";
      return;
    }
    try {
      await invoke("set_pin", { pin: newPin });
      await invoke("set_note_locked", { id: note.id, locked: true });
      onlockedchange(true);
      settingPin = false;
      newPin = "";
      newPinConfirm = "";
    } catch (e) {
      error = String(e);
    }
  }

  function onWindowPointerDown(event: PointerEvent) {
    if (settingPin && !root?.contains(event.target as Node)) settingPin = false;
  }
</script>

<svelte:window onpointerdown={onWindowPointerDown} />

<div class="lock" bind:this={root}>
  <button
    class="icon-btn"
    class:on={note.locked}
    onmousedown={(e) => e.preventDefault()}
    onclick={toggle}
    aria-pressed={note.locked}
    aria-label="Proteggi con PIN"
    disabled={note.locked && !unlocked}
    title={note.locked
      ? unlocked
        ? "Togli la protezione"
        : "Sblocca la nota per togliere la protezione"
      : "Proteggi con PIN"}
  >
    <svg viewBox="0 0 20 20" aria-hidden="true">
      <rect x="4.5" y="9" width="11" height="8" rx="2.2" />
      <path d="M6.5 9V6.5a3.5 3.5 0 0 1 7 0V9" />
    </svg>
  </button>

  {#if settingPin}
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="panel" role="dialog" aria-label="Imposta un PIN">
      <p class="hint">Imposta un PIN (4-6 cifre) per proteggere le note.</p>
      <input type="password" inputmode="numeric" maxlength="6" placeholder="PIN" bind:value={newPin} />
      <input type="password" inputmode="numeric" maxlength="6" placeholder="Conferma PIN" bind:value={newPinConfirm} />
      {#if error}<p class="error">{error}</p>{/if}
      <div class="row">
        <button class="primary" onclick={confirmNewPin}>Imposta e proteggi</button>
        <button onclick={() => (settingPin = false)}>Annulla</button>
      </div>
    </div>
  {/if}
</div>

<style>
  .lock {
    position: relative;
    display: flex;
  }
  .icon-btn {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    padding: 0;
    border: 0;
    border-radius: 8px;
    background: rgba(0, 0, 0, 0.08);
    color: rgba(0, 0, 0, 0.55);
    cursor: pointer;
  }
  .icon-btn:hover {
    background: rgba(0, 0, 0, 0.16);
  }
  .icon-btn.on {
    background: rgba(0, 0, 0, 0.78);
    color: #fff;
  }
  .icon-btn:disabled {
    cursor: default;
    opacity: 0.6;
  }
  .icon-btn svg {
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.7;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .panel {
    position: absolute;
    top: calc(100% + 8px);
    right: -40px;
    z-index: 10;
    width: 220px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px;
    border-radius: 14px;
    background: #fff;
    color: #1d1d22;
    box-shadow: 0 10px 30px rgba(0, 0, 0, 0.28);
    font: 13px "Segoe UI", system-ui, sans-serif;
  }
  .hint {
    margin: 0;
    color: #6b6b76;
  }
  .error {
    margin: 0;
    color: #c9372c;
    font-weight: 600;
  }
  input {
    width: 100%;
    box-sizing: border-box;
    padding: 6px 8px;
    border: 1px solid #d6d6de;
    border-radius: 8px;
    font: 13px "Segoe UI", system-ui, sans-serif;
    color-scheme: light;
  }
  .row {
    display: flex;
    gap: 6px;
  }
  .row button {
    flex: 1;
    padding: 6px 4px;
    border: 1px solid #e2e2e8;
    border-radius: 8px;
    background: #f6f6f8;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
    color: inherit;
    cursor: pointer;
  }
  .row .primary {
    border: 0;
    background: #3b6fe0;
    color: #fff;
  }
</style>
