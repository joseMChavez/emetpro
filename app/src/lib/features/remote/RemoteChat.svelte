<script lang="ts">
  // Mensajes con la consola, en el teléfono (remote/chat.rs): pide el nombre la primera vez, muestra en grande
  // lo que manda la consola con sus respuestas de un toque, y deja mandarle frases cortas.
  // El nombre y un identificador propio se guardan en el teléfono: al reconectarse sigue siendo la misma persona.
  import { untrack } from "svelte";
  import { t } from "$lib/i18n/index.svelte";
  import { MAX_NAME, MAX_NOTE, type PhoneChat, type PhoneMessage } from "./protocol";

  let {
    chat,
    online,
    link,
    send,
  }: {
    chat: PhoneChat | null;
    online: boolean;
    /** Cambia cada vez que se abre la conexión: hay que volver a presentarse. */
    link: number;
    send: (message: PhoneMessage) => void;
  } = $props();

  const DEVICE_STORAGE = "emetpro-remote-device";
  const NAME_STORAGE = "emetpro-remote-name";
  const TOAST_MS = 3500;

  const device = readDevice();
  let name = $state(read(NAME_STORAGE) ?? "");
  /** Pedir el nombre: la primera vez, o al tocar «Cambiar nombre». */
  let naming = $state(false);
  let draftName = $state("");
  let composing = $state(false);
  let text = $state("");
  /** Respondidos aquí: se quitan enseguida, sin esperar al servidor. */
  let answered = $state<string[]>([]);
  let toast = $state<string | null>(null);
  let toastTimer: ReturnType<typeof setTimeout> | undefined;
  const seen = new Set<string>();
  /** Frases propias que la consola aún no había leído. */
  let unread = new Set<string>();
  let skippedName = false;

  let current = $derived(chat?.pending.find((n) => !answered.includes(n.id)) ?? null);
  let waiting = $derived((chat?.pending.filter((n) => !answered.includes(n.id)).length ?? 0) - (current ? 1 : 0));

  function read(storageKey: string): string | null {
    try {
      return localStorage.getItem(storageKey);
    } catch {
      return null;
    }
  }

  function write(storageKey: string, value: string) {
    try {
      localStorage.setItem(storageKey, value);
    } catch {
      // Sin almacenamiento (modo privado): vale mientras la página siga abierta.
    }
  }

  /** crypto.randomUUID solo existe en páginas seguras (https); el remoto va por http en la red local. */
  function readDevice(): string {
    const saved = read(DEVICE_STORAGE);
    if (saved && /^[A-Za-z0-9-]{8,64}$/.test(saved)) return saved;
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    const id = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    write(DEVICE_STORAGE, id);
    return id;
  }

  // Al conectarse (y al reconectarse), presentarse; sin nombre todavía, pedirlo una vez.
  $effect(() => {
    void link;
    if (!online) return;
    if (name) send({ type: "hello", device, name });
    else if (!skippedName) askName();
  });

  // Lo que se ve en pantalla se da por visto, y avisa con una vibración (Android; el iPhone no deja).
  $effect(() => {
    const note = current;
    if (!note || seen.has(note.id) || !online) return;
    seen.add(note.id);
    send({ type: "seen", id: note.id });
    try {
      navigator.vibrate?.([200, 100, 200]);
    } catch {
      // Sin vibración.
    }
  });

  // Cuando la consola lee una frase de este teléfono, se avisa aquí.
  $effect(() => {
    const mine = chat?.mine ?? [];
    const fresh = mine.find((m) => m.read && unread.has(m.id));
    if (fresh) untrack(() => showToast(t("chat.phone.read", { text: fresh.text })));
    unread = new Set(mine.filter((m) => !m.read).map((m) => m.id));
  });

  function showToast(message: string) {
    toast = message;
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => (toast = null), TOAST_MS);
  }

  function askName() {
    draftName = name;
    naming = true;
  }

  function saveName(event: SubmitEvent) {
    event.preventDefault();
    const clean = draftName.trim().replace(/\s+/g, " ").slice(0, MAX_NAME);
    if (!clean) return;
    name = clean;
    write(NAME_STORAGE, clean);
    // Se presenta con el nombre nuevo el $effect de arriba, que depende de `name`.
    naming = false;
  }

  function skipName() {
    skippedName = true;
    naming = false;
  }

  function reply(id: string, index: number | null) {
    answered.push(id);
    send({ type: "reply", id, reply: index });
  }

  function say(message: string) {
    const clean = message.trim();
    if (!clean || !online) return;
    send({ type: "say", text: clean.slice(0, MAX_NOTE) });
    composing = false;
    text = "";
    showToast(t("chat.phone.sent"));
  }

  function openCompose() {
    if (!name) askName();
    else composing = true;
  }
</script>

<button class="btn ghost icon chat-open" onclick={openCompose} aria-label={t("chat.phone.write")} disabled={!online}>
  <svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v11H8l-4 4V5Z" /></svg>
</button>

{#if naming}
  <div class="backdrop" role="presentation">
    <div class="sheet card" role="dialog" aria-modal="true" aria-labelledby="chat-name">
    <form class="name-form" onsubmit={saveName}>
      <h2 id="chat-name">{t("chat.phone.nameTitle")}</h2>
      <p class="muted small">{t("chat.phone.nameHint")}</p>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="field" bind:value={draftName} maxlength={MAX_NAME} placeholder={t("chat.phone.namePlaceholder")} autocomplete="nickname" autofocus />
      <div class="actions">
        <button type="button" class="btn big" onclick={skipName}>{t("chat.phone.later")}</button>
        <button type="submit" class="btn primary big" disabled={!draftName.trim()}>{t("chat.phone.save")}</button>
      </div>
    </form>
    </div>
  </div>
{:else if current}
  {@const note = current}
  <!-- Lo que manda la consola: grande, para leerlo de un vistazo, y responder con un toque. -->
  <div class="backdrop" role="presentation">
    <div class="sheet card message" role="alertdialog" aria-modal="true" aria-labelledby="chat-message">
      <p class="muted small">{t(note.toAll ? "chat.phone.fromConsoleAll" : "chat.phone.fromConsole")}</p>
      <p id="chat-message" class="big-text">{note.text}</p>
      <div class="answers" class:many={note.replies.length > 2}>
        {#if note.replies.length === 0}
          <button class="btn primary big" onclick={() => reply(note.id, null)} disabled={!online}>{t("chat.ok")}</button>
        {:else}
          {#each note.replies as answer, i (i)}
            <button class="btn big" class:primary={i === 0} onclick={() => reply(note.id, i)} disabled={!online}>{answer}</button>
          {/each}
        {/if}
      </div>
      {#if waiting > 0}<p class="muted small">{t("chat.phone.more", { n: waiting })}</p>{/if}
    </div>
  </div>
{:else if composing}
  <div class="backdrop" role="presentation" onclick={() => (composing = false)}>
    <div class="sheet card" role="dialog" aria-modal="true" aria-labelledby="chat-compose" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={() => {}}>
      <div class="head">
        <h2 id="chat-compose">{t("chat.phone.toConsole")}</h2>
        <button class="btn ghost" onclick={() => (composing = false)} aria-label={t("error.dismiss")}>✕</button>
      </div>
      <div class="phrases">
        {#each chat?.phrases ?? [] as phrase, i (i)}
          <button class="btn big" onclick={() => say(phrase)} disabled={!online}>{phrase}</button>
        {/each}
      </div>
      <form class="write" onsubmit={(e) => { e.preventDefault(); say(text); }}>
        <input class="field" bind:value={text} maxlength={MAX_NOTE} placeholder={t("chat.phone.placeholder")} enterkeyhint="send" />
        <button class="btn primary" type="submit" disabled={!text.trim() || !online}>{t("chat.send")}</button>
      </form>
      {#if chat && chat.mine.length > 0}
        <ol class="mine">
          {#each chat.mine as m (m.id)}
            <li><span>{m.text}</span><span class="small" class:ok={m.read}>{m.read ? t("chat.phone.readShort") : t("chat.phone.sentShort")}</span></li>
          {/each}
        </ol>
      {/if}
      <p class="muted small who">
        {t("chat.phone.youAre", { name })}
        <button class="btn ghost small" onclick={() => { composing = false; askName(); }}>{t("chat.phone.rename")}</button>
      </p>
    </div>
  </div>
{/if}

{#if toast}<p class="toast" role="status">{toast}</p>{/if}

<style>
  .chat-open { min-height: 40px; width: 40px; padding: 0; justify-content: center; }
  .chat-open svg { width: 21px; height: 21px; fill: none; stroke: currentColor; stroke-width: 1.8; stroke-linejoin: round; }
  .phrases .btn { min-height: 52px; }
  .backdrop { position: fixed; inset: 0; z-index: 20; display: grid; place-items: center; padding: 16px; background: rgb(0 0 0 / 55%); }
  .sheet { width: min(480px, 100%); max-height: calc(100dvh - 32px); overflow: auto; padding: 18px; display: flex; flex-direction: column; gap: 12px; }
  .sheet h2 { font-size: 18px; }
  .name-form { display: flex; flex-direction: column; gap: 12px; }
  .head { display: flex; align-items: center; justify-content: space-between; }
  .actions, .answers { display: flex; gap: 10px; }
  .actions > *, .answers > * { flex: 1; }
  .answers.many { flex-direction: column; }
  .message { border: 2px solid var(--accent); }
  .big-text { font-size: 24px; font-weight: 700; line-height: 1.3; overflow-wrap: anywhere; }
  .phrases { display: grid; grid-template-columns: 1fr 1fr; gap: 8px; }
  .phrases .btn { white-space: normal; text-align: center; }
  .write { display: flex; gap: 8px; }
  .write .field { flex: 1; min-width: 0; }
  .mine { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 4px; border-top: 1px solid var(--border); padding-top: 8px; }
  .mine li { display: flex; justify-content: space-between; gap: 8px; }
  .mine .small { color: var(--muted); flex: none; }
  .mine .ok { color: var(--ok); font-weight: 600; }
  .who { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
  .toast {
    position: fixed; left: 50%; bottom: calc(150px + env(safe-area-inset-bottom)); transform: translateX(-50%); z-index: 25;
    max-width: calc(100vw - 32px); padding: 10px 16px; border-radius: 999px;
    background: var(--text); color: var(--bg); font-weight: 600; box-shadow: 0 6px 20px rgb(0 0 0 / 0.25);
  }
</style>
