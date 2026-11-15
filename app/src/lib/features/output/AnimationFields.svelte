<script lang="ts">
  // Los cuatro ajustes de una animación (D-024): en el estilo general y en la animación propia de una letra.
  import { t, type MessageKey } from "$lib/i18n/index.svelte";
  import type { TextAnimation } from "$lib/platform";
  import { AMBIENTS, BACKGROUND_MOTIONS, ENTRANCES, TRANSITIONS } from "./animation";

  let { value, onchange, id }: { value: TextAnimation; onchange: (value: TextAnimation) => void; id: string } = $props();

  const fields = [
    { key: "transition", options: TRANSITIONS },
    { key: "entrance", options: ENTRANCES },
    { key: "ambient", options: AMBIENTS },
    { key: "background", options: BACKGROUND_MOTIONS },
  ] as const;
</script>

{#each fields as field (field.key)}
  <div class="row">
    <label class="label" for="{id}-{field.key}">{t(`anim.${field.key}`)}</label>
    <select
      id="{id}-{field.key}"
      class="field compact"
      value={value[field.key]}
      onchange={(e) => onchange({ ...value, [field.key]: e.currentTarget.value })}
    >
      {#each field.options as option (option)}
        <option value={option}>{t(`anim.${field.key}.${option}` as MessageKey)}</option>
      {/each}
    </select>
  </div>
{/each}

<style>
  .row { display: flex; align-items: center; justify-content: space-between; gap: 10px; flex-wrap: wrap; }
  .label { font-size: 14px; font-weight: 600; }
  .field.compact { width: auto; min-height: 36px; padding: 4px 8px; }
</style>
