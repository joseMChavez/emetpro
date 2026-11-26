<script lang="ts">
  // Mensajes al remoto, desde la consola: a quién (uno o todos), los preparados de un toque o uno escrito en el
  // momento con sus respuestas, y cómo va cada uno (visto, respondido, sin entregar).
  import { onMount } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { ago, answerText, chat, closeNote, notePresets, people, personName, sendNote, yesNo } from "./chat.svelte";
  import { MAX_NOTE, MAX_REPLIES, MAX_REPLY, type ChatNote } from "./protocol";

  let { onclose, onsettings }: { onclose: () => void; onsettings: () => void } = $props();

  const HISTORY = 12;

  /** null = a todos. */
  let to = $state<string | null>(null);
  let text = $state("");
  let kind = $state<"ok" | "yesNo" | "custom">("ok");
  let options = $state("");
  let sending = $state(false);
  let now = $state(Date.now());
  let panel = $state<HTMLElement>();

  let list = $derived(people());
  let target = $derived(to === null ? null : (list.find((p) => p.device === to) ?? null));
  let customReplies = $derived(
    options
      .split(",")
      .map((o) => o.trim())
      .filter(Boolean),
  );
  let replies = $derived(kind === "yesNo" ? yesNo() : kind === "custom" ? customReplies : []);
  let canSend = $derived(
    list.length > 0 && text.trim().length > 0 && (kind !== "custom" || (customReplies.length >= 2 && customReplies.length <= MAX_REPLIES && customReplies.every((r) => r.length <= MAX_REPLY))),
  );
  let history = $derived([...chat.notes].reverse().slice(0, HISTORY));

  // Si la persona elegida desaparece (se apagó el control remoto), vuelve a «Todos».
  $effect(() => {
    if (to !== null && !list.some((p) => p.device === to)) to = null;
  });

  async function send(message: string, answers: string[]) {
    if (sending) return;
    sending = true;
    const ok = await sendNote(to, message, answers);
    sending = false;
    if (ok && message === text) {
      text = "";
      options = "";
      kind = "ok";
    }
  }

  function submit(event: SubmitEvent) {
    event.preventDefault();
    if (canSend) send(text, replies);
  }

  /** Cómo va un mensaje de la consola, en una línea. */
  function progress(note: ChatNote): string {
    if (note.answer) return t("chat.answered", { name: personName(note.answer.by), answer: answerText(note) });
    if (note.closed) return t("chat.withdrawn");
    if (note.to !== null && !list.find((p) => p.device === note.to)?.online) return t("chat.notDelivered", { name: personName(note.to) });
    if (note.seen.length > 0) return t("chat.seenBy", { names: note.seen.map(personName).join(", ") });
    return t("chat.waiting");
  }

  onMount(() => {
    const timer = setInterval(() => (now = Date.now()), 10_000);
    const outside = (e: PointerEvent) => {
      const el = e.target as HTMLElement;
      if (panel && !panel.contains(el) && !el.closest("[data-chat-toggle]")) onclose();
    };
    const escape = (e: KeyboardEvent) => e.key === "Escape" && onclose();
    document.addEventListener("pointerdown", outside);
    document.addEventListener("keydown", escape);
    return () => {
      clearInterval(timer);
      document.removeEventListener("pointerdown", outside);
      document.removeEventListener("keydown", escape);
    };
  });
</script>

<section class="chat card" bind:this={panel} aria-label={t("chat.title")}>
  <div class="head">
    <h2>{t("chat.title")}</h2>
    <button class="btn ghost small" onclick={onsettings}>{t("chat.settings")}</button>
    <button class="btn ghost" onclick={onclose} aria-label={t("error.dismiss")}>✕</button>
  </div>

  {#if list.length === 0}
    <p class="muted small">{t("chat.nobody")}</p>
  {:else}
    <div class="to" role="radiogroup" aria-label={t("chat.to")}>
      <span class="muted small">{t("chat.to")}</span>
      <button class="chip" role="radio" aria-checked={to === null} onclick={() => (to = null)}>{t("chat.everyone")}</button>
      {#each list as person (person.device)}
        <button class="chip" role="radio" aria-checked={to === person.device} onclick={() => (to = person.device)} title={person.online ? t("chat.online") : t("chat.offlineSince", { ago: ago(person.since, now) })}>
          <span class="dot" class:on={person.online > 0} aria-hidden="true"></span>{person.name}
        </button>
      {/each}
    </div>
    {#if target && !target.online}
      <p class="hint small">{t("chat.willDeliver", { name: target.name })}</p>
    {/if}

    <div class="presets">
      {#each notePresets().filter((p) => p.text.trim()) as preset, i (i)}
        <button class="btn preset" disabled={sending} onclick={() => send(preset.text, preset.replies)}>
          <span>{preset.text}</span>
          {#if preset.replies.length > 0}<span class="muted small">{preset.replies.join(" / ")}</span>{/if}
        </button>
      {/each}
    </div>

    <form class="compose" onsubmit={submit}>
      <input class="field" bind:value={text} maxlength={MAX_NOTE} placeholder={t("chat.placeholder")} />
      <div class="kinds" role="radiogroup" aria-label={t("chat.replies")}>
        <button type="button" class="chip" role="radio" aria-checked={kind === "ok"} onclick={() => (kind = "ok")}>{t("chat.kind.ok")}</button>
        <button type="button" class="chip" role="radio" aria-checked={kind === "yesNo"} onclick={() => (kind = "yesNo")}>{t("chat.kind.yesNo")}</button>
        <button type="button" class="chip" role="radio" aria-checked={kind === "custom"} onclick={() => (kind = "custom")}>{t("chat.kind.custom")}</button>
      </div>
      {#if kind === "custom"}
        <input class="field" bind:value={options} placeholder={t("chat.optionsPlaceholder")} />
      {/if}
      <button class="btn primary" type="submit" disabled={!canSend || sending}>{t("chat.send")}</button>
    </form>
  {/if}
  {#if chat.error}<p class="warn small" role="alert">{t(chat.error.key, chat.error.params)}</p>{/if}

  {#if history.length > 0}
    <ol class="history" aria-label={t("chat.history")}>
      {#each history as note (note.id)}
        <li class:open={!note.closed}>
          {#if note.from === null}
            <p>
              <span class="muted small">{note.to === null ? t("chat.toEveryone") : t("chat.toPerson", { name: personName(note.to) })} · {ago(note.at, now)}</span>
              <span>{note.text}</span>
            </p>
            <p class="small" class:ok={note.answer !== null}>{progress(note)}</p>
            {#if !note.closed}<button class="btn ghost small" onclick={() => closeNote(note.id)}>{t("chat.withdraw")}</button>{/if}
          {:else}
            <p>
              <span class="muted small">{personName(note.from)} · {ago(note.at, now)}</span>
              <span>💬 {note.text}</span>
            </p>
          {/if}
        </li>
      {/each}
    </ol>
  {/if}
</section>

<style>
  .chat {
    position: absolute; top: calc(100% + 8px); left: 0; z-index: 30;
    width: min(420px, calc(100vw - 32px)); max-height: min(640px, calc(100vh - 90px)); overflow: auto;
    padding: 14px; display: flex; flex-direction: column; gap: 12px;
    box-shadow: 0 10px 30px rgb(0 0 0 / 0.18);
  }
  .head { display: flex; align-items: center; gap: 6px; }
  .head h2 { font-size: 15px; flex: 1; }
  .to, .kinds { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
  .chip {
    font: inherit; font-size: 13px; padding: 4px 10px; border-radius: 999px; cursor: pointer;
    border: 1px solid var(--border); background: var(--surface); color: var(--text);
    display: inline-flex; align-items: center; gap: 6px;
  }
  .chip[aria-checked="true"] { background: var(--accent-soft); border-color: var(--accent); color: var(--text); font-weight: 600; }
  .dot { width: 8px; height: 8px; border-radius: 50%; background: var(--muted); }
  .dot.on { background: var(--ok); }
  .hint { color: var(--muted); }
  .presets { display: flex; flex-direction: column; gap: 6px; }
  .preset { display: flex; justify-content: space-between; gap: 10px; text-align: left; }
  .compose { display: flex; flex-direction: column; gap: 8px; border-top: 1px solid var(--border); padding-top: 10px; }
  .compose .primary { align-self: flex-end; }
  .warn { padding: 6px 10px; border-radius: 8px; background: var(--warn-bg); color: var(--warn-text); }
  .history { list-style: none; margin: 0; padding: 10px 0 0; border-top: 1px solid var(--border); display: flex; flex-direction: column; gap: 8px; }
  .history li { display: grid; grid-template-columns: 1fr auto; gap: 2px 8px; align-items: center; }
  .history li p:first-child { display: flex; flex-direction: column; grid-column: 1; }
  .history li p.small { grid-column: 1; color: var(--muted); }
  .history li p.ok { color: var(--ok); font-weight: 600; }
  .history li button { grid-column: 2; grid-row: 1 / span 2; }
  .history li:not(.open) { opacity: 0.8; }
</style>
