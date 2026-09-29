<script lang="ts">
  import { TEMPLATES, type Template } from "$lib/notes";

  let { onpick }: { onpick: (template: Template) => void } = $props();
</script>

<div class="templates">
  <span>Modelli</span>
  {#each TEMPLATES as template (template.label)}
    <!-- mousedown: il pulsante non ruba il focus, così la nota resta aperta -->
    <button onmousedown={(e) => e.preventDefault()} onclick={() => onpick(template)}>{template.label}</button>
  {/each}
</div>

<style>
  .templates {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
    font: 600 12px "Segoe UI", system-ui, sans-serif;
    color: rgba(0, 0, 0, 0.5);
  }
  span {
    margin-right: 2px;
  }
  button {
    padding: 3px 9px;
    border: 0;
    border-radius: 999px;
    background: rgba(0, 0, 0, 0.08);
    color: rgba(0, 0, 0, 0.7);
    font: inherit;
    cursor: pointer;
  }
  button:hover {
    background: rgba(0, 0, 0, 0.16);
  }
</style>