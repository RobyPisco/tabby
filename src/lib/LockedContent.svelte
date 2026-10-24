<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { Note } from "$lib/notes";

  let { noteId, onunlocked }: { noteId: number; onunlocked: (note: Note) => void } = $props();

  let pin = $state("");
  let error = $state("");
  let unlocking = $state(false);

  async function unlock() {
    if (pin.length < 4) return;
    error = "";
    unlocking = true;
    try {
      const note = await invoke<Note>("unlock_note", { id: noteId, pin });
      pin = "";
      onunlocked(note);
    } catch {
      error = "PIN sbagliato";
    } finally {
      unlocking = false;
    }
  }
</script>

<div class="locked">
  <p class="label">🔒 Nota protetta</p>
  <form
    class="row"
    onsubmit={(e) => {
      e.preventDefault();
      unlock();
    }}
  >
    <!-- svelte-ignore a11y_autofocus -->
    <input type="password" inputmode="numeric" maxlength="6" placeholder="PIN" bind:value={pin} autofocus />
    <button type="submit" disabled={unlocking}>Sblocca</button>
  </form>
  {#if error}<p class="error">{error}</p>{/if}
</div>

<style>
  .locked {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 18px 4px;
  }
  .label {
    margin: 0;
    font: 700 15px "Segoe UI", system-ui, sans-serif;
    color: rgba(0, 0, 0, 0.75);
  }
  .row {
    display: flex;
    gap: 6px;
  }
  input {
    flex: 1;
    box-sizing: border-box;
    padding: 6px 8px;
    border: 1px solid #d6d6de;
    border-radius: 8px;
    font: 13px "Segoe UI", system-ui, sans-serif;
    color-scheme: light;
  }
  button {
    padding: 6px 12px;
    border: 0;
    border-radius: 8px;
    background: #3b6fe0;
    color: #fff;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
    cursor: pointer;
  }
  .error {
    margin: 0;
    color: #c9372c;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
  }
</style>
