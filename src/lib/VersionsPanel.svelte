<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import NoteEditor from "$lib/NoteEditor.svelte";
  import { fullDate, relativeTime, type Version } from "$lib/notes";

  let {
    noteId,
    color,
    onclose,
    onrestored,
  }: { noteId: number; color: string; onclose: () => void; onrestored: () => void } = $props();

  function shortDate(unixSeconds: number): string {
    return new Date(unixSeconds * 1000).toLocaleString("it-IT", {
      day: "numeric",
      month: "short",
      hour: "2-digit",
      minute: "2-digit",
    });
  }

  let versions = $state<Version[]>([]);
  let loaded = $state(false);
  let chosenId = $state<number | null>(null);
  const chosen = $derived(versions.find((v) => v.id === chosenId) ?? null);

  $effect(() => {
    const id = noteId;
    loaded = false;
    invoke<Version[]>("list_versions", { noteId: id }).then((list) => {
      versions = list;
      chosenId = list[0]?.id ?? null;
      loaded = true;
    });
  });

  async function restore() {
    if (!chosen) return;
    await invoke("restore_version", { versionId: chosen.id });
    onrestored();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<section
  class="panel"
  aria-label="Cronologia della nota"
  onkeydown={(e) => {
    if (e.key === "Escape") {
      e.stopPropagation();
      onclose();
    }
  }}
>
  <header>
    <h2>Cronologia</h2>
    <button class="close" onclick={onclose} aria-label="Chiudi la cronologia">✕</button>
  </header>

  {#if !loaded}
    <p class="hint">Caricamento…</p>
  {:else if versions.length === 0}
    <p class="hint">
      Ancora nessuna versione precedente. Le versioni si salvano da sole mentre modifichi la nota,
      al massimo una ogni 10 minuti.
    </p>
  {:else}
    <ul>
      {#each versions as v (v.id)}
        <li>
          <button class:on={v.id === chosenId} onclick={() => (chosenId = v.id)} title={fullDate(v.saved_at)}>
            <span class="when">{shortDate(v.saved_at)}</span>
            <span class="ago">{relativeTime(v.saved_at)}</span>
          </button>
        </li>
      {/each}
    </ul>
    {#if chosen}
      <div class="preview" style:background={color}>
        <strong>{chosen.title || "Senza titolo"}</strong>
        {#key chosen.id}
          <NoteEditor value={chosen.body} onchange={() => {}} readonly placeholder="(vuota)" />
        {/key}
      </div>
      <button class="restore" onclick={restore}>Ripristina questa versione</button>
      <p class="hint small">Il testo attuale non si perde: diventa a sua volta una versione.</p>
    {/if}
  {/if}
</section>

<style>
  button {
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .panel {
    width: 280px;
    box-sizing: border-box;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    padding: 14px;
    border-left: 1px solid var(--line);
    background: var(--pane);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }
  h2 {
    margin: 0;
    font-size: 15px;
  }
  .close {
    width: 26px;
    height: 26px;
    border: 0;
    border-radius: 6px;
    background: transparent;
  }
  .close:hover {
    background: var(--hover);
  }
  ul {
    margin: 0;
    padding: 0;
    list-style: none;
    max-height: 180px;
    overflow-y: auto;
  }
  li button {
    width: 100%;
    display: flex;
    justify-content: space-between;
    gap: 8px;
    padding: 6px 8px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    text-align: left;
  }
  li button:hover {
    background: var(--hover);
  }
  li button.on {
    background: var(--selected);
    font-weight: 600;
  }
  .ago {
    color: var(--muted);
  }
  .preview {
    flex: 1;
    min-height: 120px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px 14px;
    border-radius: 12px;
    color: rgba(0, 0, 0, 0.85);
    --editor-size: 15px;
    --image-max-h: 120px;
  }
  .restore {
    padding: 8px;
    border: 0;
    border-radius: 8px;
    background: var(--focus);
    color: #fff;
    font-weight: 600;
  }
  .hint {
    margin: 0;
    color: var(--muted);
  }
  .small {
    font-size: 12px;
  }
</style>
