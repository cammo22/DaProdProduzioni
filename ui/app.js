/* DaProd Avatar 0.5.0 — UI vanilla, come in famiglia.
 *
 * Due modi: mini (solo avatar, senza cornice, si sposta trascinandolo; doppio
 * clic: espandi) e pannello (File, Cassetti, Converti, Modifica, Appunti,
 * Impostazioni; AI ✦ arriva per ultima).
 * I controlli della musica vivono nella striscia che appare avvicinandosi
 * all'avatar — in entrambi i modi. La scheda audio del pannello mostra solo lo
 * stato, come chiesto da Cammo.
 * Modalità eterea: clic e drag attraversano la finestra; si esce dall'icona
 * nella barra (o riaprendo DaProd).
 * L'avatar è uno spritesheet PNG+JSON (formato FoglioAvatar di DaP-Convertitore).
 */

const { invoke, convertFileSrc } = window.__TAURI__.core;
const { getCurrentWindow } = window.__TAURI__.window;

// ogni errore JS finisce nella riga di debug: mai più catene morte in silenzio
window.addEventListener("error", (e) => debug("JS: " + (e.message || e)));
window.addEventListener("unhandledrejection", (e) => {
  const r = e.reason;
  debug("JS(reject): " + (r && r.message ? r.message : r));
});

const PERSONAGGI = ["cassetta", "bobina", "cartuccia", "cd", "floppy", "minidisc", "vhs", "vinile"];
const GIRANO = new Set(["idle", "gira", "attento"]); // animazioni in loop

const DIM_MINI = [210, 266];
const DIM_PANNELLO = [580, 680];

const FORMATI = {
  immagine: ["png", "jpg", "webp"],
  video: ["mp4", "webm", "mp3", "wav"],
  audio: ["mp3", "wav", "flac", "m4a"],
};
// formato proposto di default quando un file entra in Converti
const DEFAULT_FORMATO = { immagine: "jpg", audio: "mp3", video: "mp4" };
// ---------------------------------------------------------------- avatar (spritesheet)

class Avatar {
  constructor(canvas) {
    this.canvas = canvas;
    this.ctx = canvas.getContext("2d");
    this.foglio = null;
    this.nome = null;
    this.anim = null;
    this.inizio = 0;
    this.dopo = null;
    this.congelato = false;
    this.oraFissa = 0;
    this._disegna = this._disegna.bind(this);
    requestAnimationFrame(this._disegna);
  }

  // pausa vera: l'animazione si ferma sul fotogramma corrente, niente convulsioni
  congela() {
    if (!this.congelato) {
      this.congelato = true;
      this.oraFissa = performance.now();
    }
  }

  scongela() {
    if (this.congelato) {
      this.inizio += performance.now() - this.oraFissa;
      this.congelato = false;
    }
  }

  async scegli(nome) {
    const [img, meta] = await Promise.all([
      new Promise((ok, ko) => {
        const i = new Image();
        i.onload = () => ok(i);
        i.onerror = ko;
        i.src = `avatar/${nome}.png`;
      }),
      fetch(`avatar/${nome}.json`).then((r) => r.json()),
    ]);
    this.foglio = { img, ...meta };
    this.nome = nome;
  }

  suona(nome, dopo = null) {
    if (!this.foglio || !this.foglio.animazioni[nome]) return;
    this.anim = nome;
    this.inizio = performance.now();
    this.dopo = dopo;
  }

  _disegna(oraReale) {
    requestAnimationFrame(this._disegna);
    const ora = this.congelato ? this.oraFissa : oraReale;
    const c = this.ctx;
    const L = this.canvas.width;
    c.clearRect(0, 0, L, L);
    if (!this.foglio || !this.anim) return;

    const f = this.foglio;
    const a = f.animazioni[this.anim];
    if (!a) return;
    const idx = Math.floor(((ora - this.inizio) / 1000) * f.fps);

    if (idx >= a.n) {
      if (GIRANO.has(this.anim)) {
        this.inizio = ora - ((ora - this.inizio) % ((a.n / f.fps) * 1000));
      } else {
        const prossimo = this.dopo;
        this.dopo = null;
        this.suona(prossimo && f.animazioni[prossimo] ? prossimo : "idle");
        return;
      }
    }

    const i = a.da + (idx % a.n);
    const col = i % f.colonne;
    const riga = Math.floor(i / f.colonne);
    c.drawImage(f.img, col * f.lato, riga * f.lato, f.lato, f.lato, 0, 0, L, L);
  }
}

// ---------------------------------------------------------------- stato

let avatar = null;
let schedaAudio = null;
let suonando = false;
let inPausa = false;
let trascinando = false;
let modo = "mini";
let tabAttiva = "file";
let eterea = false;

let cassetti = [];
let cassettoAttivo = null;
let listaConv = []; // { info, formato, targetMb, esito, errore }
let fileModifica = null; // InfoFile
let appunti = [];
let impostazioni = { scorciatoiaScreenshot: null, eterea: false, sempreSopra: true };

const $ = (id) => document.getElementById(id);

function debug(msg) {
  $("debug").textContent = String(msg);
  try { invoke("traccia_debug", { msg: String(msg) }).catch(() => {}); } catch {}
}

function visoDiStato() {
  if (trascinando) { avatar.scongela(); return avatar.suona("attento"); }
  if (suonando && !inPausa) { avatar.scongela(); return avatar.suona("gira"); }
  if (suonando && inPausa) return avatar.congela(); // in pausa l'avatar si ferma
  avatar.scongela();
  avatar.suona("idle");
}

function fmtTempo(s) {
  if (s == null || !isFinite(s)) return "--:--";
  const m = Math.floor(s / 60);
  const sec = Math.floor(s % 60);
  return `${m}:${String(sec).padStart(2, "0")}`;
}

function fmtByte(b) {
  if (b < 1024) return `${b} B`;
  if (b < 1024 * 1024) return `${(b / 1024).toFixed(1)} KB`;
  if (b < 1024 * 1024 * 1024) return `${(b / 1024 / 1024).toFixed(1)} MB`;
  return `${(b / 1024 / 1024 / 1024).toFixed(2)} GB`;
}

async function saltaRelativo(delta) {
  try {
    const st = await invoke("stato_audio");
    if (!st.inCorso) return;
    const np = await invoke("salta_a", { posizione: Math.max(0, st.posizione + delta) });
    debug(`saltato a ${fmtTempo(np)}`);
  } catch (e) {
    debug(e);
  }
}

function tipoDaKind(kind) {
  return { image: "immagine", video: "video", audio: "audio", text: "testo" }[kind] || "altro";
}

// ---------------------------------------------------------------- icone (dal Convertitore di famiglia)

function ic(id, svg) {
  const e = $(id);
  if (e) e.innerHTML = svg;
}

function applicaIcone() {
  // barra
  ic("cambia", ICONE.avatar);
  ic("eterea", ICONE.occhio);
  ic("impostazioni", ICONE.ingranaggio);
  ic("espandi", ICONE.ingrandisci);
  ic("chiudi", ICONE.chiudi);
  // striscia controlli: stessi glifi del mini player del Convertitore
  ic("ca-indietro", ICONE_LETTORE.indietro10);
  ic("ca-avanti", ICONE_LETTORE.avanti10);
  ic("ca-gioca", ICONE_LETTORE.pausa);
  ic("ca-stop", ICONE.ferma);
  ic("ca-screenshot", ICONE_LETTORE.foto);
  ic("ca-eterea", ICONE.occhio);
  ic("ca-impostazioni", ICONE.ingranaggio);
  ic("ca-espandi", ICONE_LETTORE.finestra);
  // cassetti e appunti
  ic("nuovo-cassetto", ICONE.piu);
  ic("elimina-cassetto", ICONE.cestino);
  ic("apri-cartella", ICONE.cartella);
  ic("svuota-appunti", ICONE.cestino);
  // logo di famiglia in barra
  const t = document.querySelector(".barra .titolo");
  if (t && !t.querySelector("svg")) {
    t.insertAdjacentHTML("afterbegin", `<span class="logo-barra">${LOGO_SVG}</span>`);
  }
}

// categoria alla DaP-Convertitore: glifo e tinta per gruppo di file
function categoriaDi(f) {
  const k = { image: "immagine", video: "video", audio: "audio", text: "testo" }[f.kind];
  if (k) return k;
  const est = (f.extension || "").toLowerCase();
  if (est === "pdf") return "pdf";
  if (["doc", "docx", "odt", "rtf", "wpd"].includes(est)) return "documento";
  if (["xls", "xlsx", "ods", "numbers"].includes(est)) return "foglio";
  if (["ppt", "pptx", "odp"].includes(est)) return "presentazione";
  if (["zip", "7z", "rar", "tar", "gz", "xz", "bz2", "zst", "iso"].includes(est)) return "archivio";
  if (["json", "xml", "yaml", "yml", "csv", "tsv"].includes(est)) return "dati";
  if (["srt", "vtt", "ass", "ssa"].includes(est)) return "sottotitoli";
  return "altro";
}

// ---------------------------------------------------------------- modi e finestra

async function impostaDimensione(l, a) {
  const win = getCurrentWindow();
  const dpi = window.__TAURI__.dpi;
  try {
    if (dpi && dpi.LogicalSize) {
      await win.setSize(new dpi.LogicalSize(l, a));
      return;
    }
    const scale = await win.scaleFactor();
    await win.setSize({ Physical: { width: Math.round(l * scale), height: Math.round(a * scale) } });
  } catch (e) {
    debug("ridimensionamento fallito: " + e);
  }
}

function applicaModo() {
  document.body.dataset.modo = modo;
  $("espandi").innerHTML = modo === "mini" ? ICONE.ingrandisci : ICONE.riduci;
  impostaDimensione(...(modo === "mini" ? DIM_MINI : DIM_PANNELLO));
}

async function espandi() {
  if (modo === "pannello") return;
  modo = "pannello";
  applicaModo();
  await riposizionaPerModo();
  await selezionaTab(tabAttiva || "file");
}

function riduci() {
  modo = "mini";
  applicaModo();
  riposizionaPerModo();
}

// tiene la finestra dentro lo schermo quando cambia larghezza
async function riposizionaPerModo() {
  try {
    const win = getCurrentWindow();
    const mon = await win.currentMonitor();
    if (!mon) return;
    const pos = await win.outerPosition();
    const [w] = modo === "mini" ? DIM_MINI : DIM_PANNELLO;
    const maxX = mon.size.width - w - 8;
    const x = Math.max(0, Math.min(pos.x, maxX));
    const y = Math.max(0, Math.min(pos.y, mon.size.height - 120));
    const dpi = window.__TAURI__.dpi;
    if (dpi && dpi.PhysicalPosition) await win.setPosition(new dpi.PhysicalPosition(x, y));
  } catch { /* resta dove sta */ }
}

async function selezionaTab(nome) {
  tabAttiva = nome;
  document.querySelectorAll("#tabs button").forEach((b) => b.classList.toggle("attivo", b.dataset.tab === nome));
  document.querySelectorAll(".pag-tab").forEach((p) => p.classList.toggle("attiva", p.id === "tab-" + nome));
  if (nome === "cassetti") await caricaCassetti();
  if (nome === "converti") disegnaConversioni();
  if (nome === "modifica") disegnaEditor();
  if (nome === "appunti") await caricaAppunti();
  if (nome === "impostazioni") await apriImpostazioni();
}

// ---------------------------------------------------------------- modalità eterea

async function applicaEterea(attiva, senzaConferma = false) {
  if (attiva && !senzaConferma) {
    const ok = confirm(
      "Modalità eterea: clic e drag attraverseranno l'avatar.\n" +
      "Per disattivarla: clicca l'icona di DaProd nella barra delle applicazioni\n" +
      "(se non la vedi, chiudi e riapri DaProd).\n\nAttivare?"
    );
    if (!ok) { $("imp-eterea").checked = false; return; }
  }
  eterea = attiva;
  impostazioni.eterea = attiva;
  document.body.classList.toggle("eterea", attiva);
  try {
    const win = getCurrentWindow();
    await win.setIgnoreCursorEvents(attiva);
    if (attiva) {
      await win.setAlwaysOnTop(true); // il fantasma resta visibile
      await win.setFocus(false); // niente focus: altrimenti l'escape-hatch la spegne subito
    }
    await invoke("salva_impostazioni", { imp: impostazioni });
    debug(attiva ? "modalità eterea attiva — esci dall'icona nella barra" : "modalità eterea disattivata");
  } catch (e) {
    debug("eterea: " + e);
  }
}

// ---------------------------------------------------------------- scheda audio (solo stato: i comandi stanno sulla striscia)

function intestazione(info, conAzioni) {
  const riga = document.createElement("div");
  riga.className = "riga-titolo";
  const est = document.createElement("span");
  est.className = "est";
  est.textContent = info.estensione || "file";
  const nome = document.createElement("span");
  nome.className = "nome";
  nome.textContent = info.nome;
  nome.title = info.percorso;
  riga.append(est, nome);
  if (conAzioni) {
    const conv = document.createElement("button");
    conv.className = "azione-scheda";
    conv.textContent = "Converti";
    conv.title = "Manda al tab Converti";
    conv.onclick = async () => { aggiungiAConverti(info); await espandi(); selezionaTab("converti"); };
    const mod = document.createElement("button");
    mod.className = "azione-scheda";
    mod.textContent = "Modifica";
    mod.title = "Manda al tab Modifica";
    mod.onclick = async () => { fileModifica = info; await espandi(); selezionaTab("modifica"); };
    if (FORMATI[info.tipo]) riga.appendChild(conv);
    if (["video", "audio", "immagine", "testo"].includes(info.tipo)) riga.appendChild(mod);
  }
  const chiudi = document.createElement("button");
  chiudi.className = "chiudi-scheda";
  chiudi.innerHTML = "&times;";
  riga.appendChild(chiudi);
  return { riga, chiudi };
}

function schedaBase(info, conAzioni = true) {
  const s = document.createElement("div");
  s.className = "scheda";
  s.dataset.tipo = info.tipo;
  const { riga, chiudi } = intestazione(info, conAzioni);
  s.appendChild(riga);
  chiudi.onclick = () => {
    if (s === schedaAudio) {
      schedaAudio = null;
      invoke("ferma").catch(() => {});
      suonando = false;
      inPausa = false;
      document.body.classList.remove("suona");
      visoDiStato();
    }
    s.remove();
  };
  $("schede").prepend(s);
  return s;
}

async function schedaAudioCrea(info) {
  if (schedaAudio) {
    schedaAudio.remove();
    schedaAudio = null;
  }
  let brano;
  try {
    brano = await invoke("riproduci", { percorso: info.percorso });
  } catch (e) {
    await espandi();
    return schedaErrore(info, e);
  }
  suonando = true;
  inPausa = false;
  document.body.classList.add("suona");
  debug("riproduco: " + info.nome);

  const s = schedaBase(info);
  schedaAudio = s;

  // i comandi stanno sulla striscia dell'avatar (mini e pannello): qui solo lo stato
  const corpo = document.createElement("div");
  corpo.className = "audio-corpo";
  const tempi = document.createElement("span");
  tempi.className = "tempi";
  tempi.textContent = `0:00 / ${fmtTempo(brano.durata)} — comandi: passa vicino all'avatar`;
  corpo.appendChild(tempi);
  s.appendChild(corpo);
  s._aggiorna = (st) => {
    tempi.textContent = `${fmtTempo(st.posizione)} / ${fmtTempo(st.durata)}${st.inPausa ? " — in pausa" : ""}`;
  };
  visoDiStato();
}

function schedaVideo(info) {
  const s = schedaBase(info);
  const v = document.createElement("video");
  v.controls = true;
  v.autoplay = true;
  v.src = convertFileSrc(info.percorso);
  v.onerror = () => {
    v.remove();
    const p = document.createElement("div");
    p.className = "errore";
    p.textContent = "Il webview non decodifica questo video. Prova ad aprirlo col programma predefinito.";
    const b = bottoneApri(info.percorso);
    s.append(p, b);
  };
  s.appendChild(v);
  if (!suonando) avatar.suona("saluta");
}

function schedaImmagine(info) {
  const s = schedaBase(info);
  const img = document.createElement("img");
  img.className = "anteprima";
  img.src = convertFileSrc(info.percorso);
  img.alt = info.nome;
  img.title = "Clic: apri col programma predefinito";
  img.onclick = () => invoke("apri_esterno", { percorso: info.percorso }).catch(() => {});
  img.onerror = () => {
    img.remove();
    const p = document.createElement("div");
    p.className = "errore";
    p.textContent = "Immagine non decodificabile nella webview.";
    s.appendChild(p);
  };
  s.appendChild(img);
  if (!suonando) avatar.suona("saluta");
}

async function schedaTesto(info) {
  const s = schedaBase(info);
  let letto;
  try {
    letto = await invoke("leggi_testo", { percorso: info.percorso, limiteKb: 256 });
  } catch (e) {
    return schedaErrore(info, e);
  }
  const pre = document.createElement("pre");
  pre.className = "testo";
  pre.textContent = letto.contenuto;
  s.appendChild(pre);
  if (letto.troncato) {
    const t = document.createElement("div");
    t.className = "troncato";
    t.textContent = `Mostrati i primi 256 KB su ${fmtByte(info.byte)}.`;
    s.appendChild(t);
  }
  if (!suonando) avatar.suona("saluta");
}

function bottoneApri(percorso, etichetta = "Apri col programma predefinito") {
  const b = document.createElement("button");
  b.className = "pulsante-verde";
  b.textContent = etichetta;
  b.style.marginTop = "8px";
  b.onclick = () => invoke("apri_esterno", { percorso }).catch((e) => debug(e));
  return b;
}

function schedaAltro(info) {
  const s = schedaBase(info);
  const corpo = document.createElement("div");
  corpo.className = "altro-corpo";
  const meta = document.createElement("span");
  meta.className = "meta";
  meta.textContent = fmtByte(info.byte);
  const b = bottoneApri(info.percorso);
  b.style.marginTop = "0";
  corpo.append(meta, b);
  s.appendChild(corpo);
  if (!suonando) avatar.suona("saluta");
}

function schedaErrore(info, messaggio) {
  const s = schedaBase(info, false);
  const p = document.createElement("div");
  p.className = "errore";
  p.textContent = String(messaggio);
  s.appendChild(p);
  avatar.suona("attento");
  debug("errore: " + messaggio);
  return s;
}

function schedaPerTipo(info) {
  if (info.tipo === "video") schedaVideo(info);
  else if (info.tipo === "immagine") schedaImmagine(info);
  else if (info.tipo === "testo") schedaTesto(info);
  else schedaAltro(info);
}

// ---------------------------------------------------------------- instradamento drop

async function instrada(paths) {
  if (!paths || !paths.length) {
    debug("drop arrivato con 0 percorsi");
    return;
  }
  debug(`drop: ${paths.length} file`);

  // nel pannello, il drop va al tab attivo
  if (modo === "pannello" && tabAttiva === "cassetti") {
    await importaNelCassetto(paths);
    return;
  }
  if (modo === "pannello" && tabAttiva === "converti") {
    for (const p of paths) {
      try { aggiungiAConverti(await invoke("info_file", { percorso: p })); }
      catch (e) { debug(e); }
    }
    return;
  }
  if (modo === "pannello" && tabAttiva === "modifica") {
    try { fileModifica = await invoke("info_file", { percorso: paths[0] }); disegnaEditor(); }
    catch (e) { debug(e); }
    return;
  }

  // mini, oppure tab File: audio subito, gli altri aprono il pannello
  for (const p of paths) {
    let info;
    try {
      info = await invoke("info_file", { percorso: p });
    } catch (e) {
      await espandi();
      schedaErrore({ percorso: p, nome: p.split("/").pop(), estensione: "", byte: 0, tipo: "altro" }, e);
      continue;
    }
    if (info.tipo === "audio") {
      await schedaAudioCrea(info);
    } else {
      await espandi();
      schedaPerTipo(info);
    }
  }
}

// ---------------------------------------------------------------- cassetti

async function caricaCassetti() {
  try {
    cassetti = await invoke("list_cassetti");
  } catch (e) {
    debug("cassetti: " + e);
    return;
  }
  if (!cassettoAttivo || !cassetti.some((c) => c.id === cassettoAttivo)) {
    cassettoAttivo = (cassetti.find((c) => c.id === "In arrivo") || cassetti[0] || {}).id || null;
  }
  disegnaCassetti();
  await caricaFileCassetto();
}

function disegnaCassetti() {
  const cont = $("lista-cassetti");
  cont.innerHTML = "";
  for (const c of cassetti) {
    const chip = document.createElement("button");
    chip.className = "cassetto" + (c.id === cassettoAttivo ? " attivo" : "");
    chip.textContent = `${c.label || "(senza nome)"} · ${c.count}`;
    chip.title = "Clic: apri — doppio clic: rinomina";
    chip.onclick = () => { cassettoAttivo = c.id; disegnaCassetti(); caricaFileCassetto(); };
    chip.ondblclick = () => rinominaInline(chip, c);
    cont.appendChild(chip);
  }
}

function rinominaInline(chip, c) {
  const input = document.createElement("input");
  input.className = "rinomina";
  input.value = c.label || "";
  input.placeholder = "nome del cassetto";
  chip.replaceWith(input);
  input.focus();
  let chiuso = false;
  const chiudi = async () => {
    if (chiuso) return;
    chiuso = true;
    const nuovo = input.value.trim();
    try {
      await invoke("rename_cassetto", { id: c.id, label: nuovo });
    } catch (e) {
      debug("rinomina: " + e);
    }
    await caricaCassetti();
  };
  input.onblur = chiudi;
  input.onkeydown = (e) => {
    if (e.key === "Enter") input.blur();
    if (e.key === "Escape") { chiuso = true; caricaCassetti(); }
  };
}

async function caricaFileCassetto() {
  const attivo = cassetti.find((c) => c.id === cassettoAttivo);
  $("nome-cassetto").textContent = attivo ? (attivo.label || "(senza nome)") : "nessun cassetto";
  const g = $("griglia-file");
  g.innerHTML = "";
  if (!cassettoAttivo) return;
  let file;
  try {
    file = await invoke("list_files", { drawer: cassettoAttivo });
  } catch (e) {
    debug("file: " + e);
    return;
  }
  if (!file.length) {
    g.innerHTML = '<div class="vuoto-cassetto">Cassetto vuoto — trascina qui i file per copiarli</div>';
    return;
  }
  for (const f of file) {
    const card = document.createElement("div");
    card.className = "elemento";

    if (f.thumb) {
      const img = document.createElement("img");
      img.src = f.thumb;
      img.title = "Apri";
      img.onclick = () => invoke("apri_esterno", { percorso: f.path }).catch((e) => debug(e));
      card.appendChild(img);
    } else {
      const badge = document.createElement("div");
      badge.className = "badge-est";
      const cat = categoriaDi(f);
      badge.innerHTML = ICONA_CATEGORIA[cat] || ICONA_CATEGORIA.altro;
      badge.style.color = COLORE_CATEGORIA[cat] || "var(--oro)";
      badge.title = f.extension || "?";
      card.appendChild(badge);
    }

    const nome = document.createElement("div");
    nome.className = "nome-file";
    nome.textContent = f.name;
    nome.title = `${f.path} — ${fmtByte(f.size)}`;
    card.appendChild(nome);

    const info = {
      percorso: f.path, nome: f.name,
      estensione: (f.extension || "").toLowerCase(),
      byte: f.size, tipo: tipoDaKind(f.kind),
    };

    const azioni = document.createElement("div");
    azioni.className = "azioni-elemento";
    const mk = (testo, titolo, fn, cls) => {
      const b = document.createElement("button");
      b.textContent = testo;
      b.title = titolo;
      if (cls) b.className = cls;
      b.onclick = fn;
      return b;
    };
    azioni.append(
      mk("\u2197", "Apri col programma predefinito", () => invoke("apri_esterno", { percorso: f.path }).catch((e) => debug(e))),
      mk("\u21C4", "Manda a Converti", () => { aggiungiAConverti(info); selezionaTab("converti"); }),
      mk("\u270E", "Manda a Modifica", () => { fileModifica = info; selezionaTab("modifica"); }),
      mk("\uD83D\uDDD1", "Elimina (solo libreria)", async () => {
        if (!confirm(`Eliminare «${f.name}» dal cassetto?`)) return;
        try { await invoke("elimina_se_in_libreria", { path: f.path }); await caricaCassetti(); }
        catch (e) { debug(e); }
      }, "elimina"),
    );
    card.appendChild(azioni);
    g.appendChild(card);
  }
}

async function importaNelCassetto(paths) {
  if (!cassettoAttivo) { debug("nessun cassetto aperto"); return; }
  try {
    const salvati = await invoke("import_paths", { paths, drawer: cassettoAttivo });
    debug(`copiati ${salvati.length} file nel cassetto`);
    await caricaCassetti();
  } catch (e) {
    debug("import: " + e);
  }
}

// ---------------------------------------------------------------- converti (slider del peso)

function aggiungiAConverti(info) {
  if (!info) return;
  if (!FORMATI[info.tipo]) {
    debug(`«${info.nome}» non è convertibile (solo immagini, audio, video)`);
    return;
  }
  if (listaConv.some((x) => x.info.percorso === info.percorso)) return;
  listaConv.push({ info, formato: DEFAULT_FORMATO[info.tipo] || FORMATI[info.tipo][0], targetMb: null, esito: null, errore: null });
  disegnaConversioni();
}

function disegnaConversioni() {
  const cont = $("lista-conversione");
  cont.innerHTML = "";
  listaConv.forEach((elem, idx) => {
    const blocco = document.createElement("div");

    const riga = document.createElement("div");
    riga.className = "riga-conv";

    const nome = document.createElement("span");
    nome.className = "nome";
    nome.textContent = `${elem.info.nome} · ${fmtByte(elem.info.byte)}`;
    nome.title = elem.info.percorso;

    const chips = document.createElement("div");
    chips.className = "chips-formato";
    for (const f of FORMATI[elem.info.tipo]) {
      const c = document.createElement("button");
      c.className = "chip" + (f === elem.formato ? " attiva" : "");
      c.textContent = "." + f;
      c.title = "Converti in ." + f;
      c.onclick = () => { elem.formato = f; disegnaConversioni(); };
      chips.appendChild(c);
    }

    const btn = document.createElement("button");
    btn.textContent = "Converti";
    btn.onclick = async () => {
      btn.disabled = true;
      btn.textContent = "\u2026";
      elem.errore = null;
      elem.esito = null;
      try {
        const targetKb = elem.targetMb ? Math.round(elem.targetMb * 1024) : null;
        elem.esito = await invoke("convert_file", { path: elem.info.percorso, format: elem.formato, targetKb });
        debug("convertito: " + elem.esito);
      } catch (e) {
        elem.errore = String(e);
        debug("conversione fallita: " + e);
      }
      disegnaConversioni();
    };

    const togli = document.createElement("button");
    togli.className = "togli";
    togli.innerHTML = "&times;";
    togli.title = "Togli dalla lista";
    togli.onclick = () => { listaConv.splice(idx, 1); disegnaConversioni(); };

    riga.append(nome, chips, btn, togli);
    blocco.appendChild(riga);

    // slider del peso massimo: da 0,1 MB al peso originale (tutto a destra = nessun limite)
    const mbOriginale = Math.max(0.1, elem.info.byte / (1024 * 1024));
    const slider = document.createElement("div");
    slider.className = "slider-peso";
    const rng = document.createElement("input");
    rng.type = "range";
    rng.min = "0.1";
    rng.max = String(Math.round(mbOriginale * 10) / 10);
    rng.step = "0.1";
    rng.value = String(elem.targetMb ?? Math.round(mbOriginale * 10) / 10);
    const val = document.createElement("span");
    val.className = "val-peso";
    const aggiornaEtichetta = () => {
      const v = parseFloat(rng.value);
      if (v >= mbOriginale - 0.05) {
        elem.targetMb = null;
        val.textContent = `nessun limite (${fmtByte(elem.info.byte)})`;
      } else {
        elem.targetMb = v;
        val.textContent = `max ${v.toFixed(1)} MB`;
      }
    };
    rng.oninput = aggiornaEtichetta;
    aggiornaEtichetta();
    slider.append(rng, val);
    riga.appendChild(slider);

    if (elem.esito) {
      const es = document.createElement("div");
      es.className = "esito-conv";
      es.textContent = "fatto: calcolo il peso\u2026";
      const percorsoEsito = elem.esito;
      blocco.appendChild(es);
      invoke("info_file", { percorso: percorsoEsito })
        .then((out) => {
          es.textContent = "";
          es.append("fatto: ");
          const a = document.createElement("a");
          a.textContent = `${percorsoEsito.split("/").pop()} (${fmtByte(out.byte)})`;
          a.title = percorsoEsito;
          a.onclick = () => invoke("apri_esterno", { percorso: percorsoEsito }).catch((e) => debug(e));
          es.appendChild(a);
        })
        .catch(() => { es.textContent = "fatto: " + percorsoEsito; });
    }
    if (elem.errore) {
      const er = document.createElement("div");
      er.className = "errore";
      er.textContent = elem.errore;
      blocco.appendChild(er);
    }
    cont.appendChild(blocco);
  });
}

// ---------------------------------------------------------------- modifica

function disegnaEditor() {
  const cont = $("editor");
  cont.innerHTML = "";
  const info = fileModifica;
  if (!info) {
    cont.innerHTML = '<div class="suggerimento">Nessun file scelto. Trascinalo qui, oppure usa \u270E nei cassetti o \u00ABModifica\u00BB nelle schede.</div>';
    return;
  }

  const scheda = document.createElement("div");
  scheda.className = "editor-scheda";

  const testa = document.createElement("div");
  testa.className = "riga-titolo";
  const est = document.createElement("span");
  est.className = "est";
  est.textContent = info.estensione || "file";
  const nome = document.createElement("span");
  nome.className = "nome";
  nome.textContent = info.nome;
  nome.title = info.percorso;
  testa.append(est, nome);
  scheda.appendChild(testa);

  const rigaEsito = (percorso) => {
    const es = document.createElement("div");
    es.className = "esito-conv";
    const a = document.createElement("a");
    a.textContent = percorso.split("/").pop();
    a.title = percorso;
    a.onclick = () => invoke("apri_esterno", { percorso }).catch((e) => debug(e));
    es.append("fatto: ", a);
    scheda.appendChild(es);
  };
  const rigaErrore = (e) => {
    const er = document.createElement("div");
    er.className = "errore";
    er.textContent = String(e);
    scheda.appendChild(er);
  };

  if (info.tipo === "video" || info.tipo === "audio") {
    const riga = document.createElement("div");
    riga.className = "editor-riga";
    const li = document.createElement("label");
    li.textContent = "da (s)";
    const inizio = document.createElement("input");
    inizio.type = "number";
    inizio.min = "0";
    inizio.step = "0.1";
    inizio.value = "0";
    const lf = document.createElement("label");
    lf.textContent = "a (s)";
    const fine = document.createElement("input");
    fine.type = "number";
    fine.min = "0";
    fine.step = "0.1";
    fine.value = "10";
    const btn = document.createElement("button");
    btn.textContent = "Taglia";
    btn.onclick = async () => {
      btn.disabled = true;
      btn.textContent = "\u2026";
      try {
        const out = await invoke("trim_media", {
          path: info.percorso,
          start: parseFloat(inizio.value) || 0,
          end: parseFloat(fine.value) || 0,
        });
        debug("tagliato: " + out);
        rigaEsito(out);
        if (cassettoAttivo) caricaFileCassetto();
      } catch (e) {
        rigaErrore(e);
      }
      btn.disabled = false;
      btn.textContent = "Taglia";
    };
    riga.append(li, inizio, lf, fine, btn);
    scheda.appendChild(riga);
    const nota = document.createElement("div");
    nota.className = "suggerimento";
    nota.style.textAlign = "left";
    nota.textContent = info.tipo === "video"
      ? "Il taglio produce un nuovo .mp4 accanto all'originale (l'originale non si tocca)."
      : "Il taglio produce un nuovo .m4a accanto all'originale (l'originale non si tocca).";
    scheda.appendChild(nota);
  } else if (info.tipo === "immagine") {
    const riga = document.createElement("div");
    riga.className = "editor-riga";
    const btn = document.createElement("button");
    btn.textContent = "Ruota di 90\u00B0";
    btn.onclick = async () => {
      try {
        const out = await invoke("rotate_image", { path: info.percorso });
        debug("ruotata: " + out);
        rigaEsito(out);
        if (cassettoAttivo) caricaFileCassetto();
      } catch (e) { rigaErrore(e); }
    };
    const anteprima = document.createElement("img");
    anteprima.className = "anteprima";
    anteprima.style.maxHeight = "180px";
    anteprima.src = convertFileSrc(info.percorso);
    riga.appendChild(btn);
    scheda.append(riga, anteprima);
  } else if (info.tipo === "testo") {
    const area = document.createElement("textarea");
    area.className = "edit-testo";
    area.value = "carico\u2026";
    scheda.appendChild(area);
    invoke("read_testo", { path: info.percorso })
      .then((t) => { area.value = t; })
      .catch((e) => { area.value = ""; rigaErrore(e); });
    const riga = document.createElement("div");
    riga.className = "editor-riga";
    const btn = document.createElement("button");
    btn.textContent = "Salva";
    btn.onclick = async () => {
      try {
        await invoke("write_testo", { path: info.percorso, text: area.value });
        debug("salvato: " + info.nome);
      } catch (e) { rigaErrore(e); }
    };
    riga.appendChild(btn);
    scheda.appendChild(riga);
  } else {
    scheda.appendChild(bottoneApri(info.percorso));
  }

  cont.appendChild(scheda);
}

// ---------------------------------------------------------------- appunti (storia dei copia)

function fmtQuando(id) {
  try {
    const d = new Date(id);
    const oggi = new Date();
    const ora = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
    if (d.toDateString() === oggi.toDateString()) return ora;
    return `${d.getDate()}/${d.getMonth() + 1} ${ora}`;
  } catch { return ""; }
}

async function caricaAppunti() {
  try {
    appunti = await invoke("list_appunti");
  } catch (e) {
    debug("appunti: " + e);
    return;
  }
  disegnaAppunti();
}

function disegnaAppunti() {
  const cont = $("lista-appunti");
  cont.innerHTML = "";
  if (!appunti.length) {
    cont.innerHTML = '<div class="appunti-vuoto">Niente in memoria — copia qualcosa (Ctrl+C) e apparirà qui</div>';
    return;
  }
  for (const p of appunti) {
    const riga = document.createElement("div");
    riga.className = "appunto";
    riga.title = "Clic: rimetti negli appunti";
    const testo = document.createElement("span");
    testo.className = "testo-appunto";
    testo.textContent = p.testo;
    const quando = document.createElement("span");
    quando.className = "quando";
    quando.textContent = fmtQuando(p.id);
    const togl = document.createElement("button");
    togl.className = "togli-appunto";
    togl.innerHTML = "&times;";
    togl.title = "Elimina dalla storia";
    togl.onclick = async (e) => {
      e.stopPropagation();
      try { await invoke("elimina_appunto", { id: p.id }); await caricaAppunti(); }
      catch (err) { debug(err); }
    };
    riga.onclick = async () => {
      try {
        await invoke("copia_appunto", { testo: p.testo });
        debug("appunto rimesso negli appunti");
      } catch (e) { debug(e); }
    };
    riga.append(testo, quando, togl);
    cont.appendChild(riga);
  }
}

// ---------------------------------------------------------------- impostazioni

async function apriImpostazioni() {
  try {
    impostazioni = await invoke("leggi_impostazioni");
  } catch (e) {
    debug("impostazioni: " + e);
  }
  $("imp-scorciatoia-attuale").textContent = impostazioni.scorciatoiaScreenshot || "nessuna";
  $("imp-eterea").checked = !!impostazioni.eterea;
  $("imp-sopra").checked = impostazioni.sempreSopra !== false;
}

function chiaveCombo(e) {
  // modificatori da soli e Escape (annulla) non contano
  if (["Shift", "Control", "Alt", "Meta", "Escape"].includes(e.key)) return null;
  const parti = [];
  if (e.ctrlKey) parti.push("Ctrl");
  if (e.altKey) parti.push("Alt");
  if (e.shiftKey) parti.push("Shift");
  if (e.metaKey) parti.push("Meta");
  let tasto;
  if (e.key === " ") tasto = "space";
  else if (e.key.length === 1) tasto = e.key.toUpperCase();
  else tasto = e.key; // Delete, F5, PageUp, ArrowLeft… va bene tutto
  parti.push(tasto);
  return parti.join("+");
}

async function registraScorciatoia() {
  const etichetta = $("imp-scorciatoia-attuale");
  const btn = $("imp-registra");
  etichetta.textContent = "premi la combinazione…";
  etichetta.classList.add("imp-registra-attesa");
  btn.disabled = true;
  const cleanup = () => {
    window.removeEventListener("keydown", onKey, true);
    etichetta.classList.remove("imp-registra-attesa");
    btn.disabled = false;
  };
  const onKey = async (e) => {
    e.preventDefault();
    e.stopPropagation();
    if (e.key === "Escape") { cleanup(); await apriImpostazioni(); return; }
    const combo = chiaveCombo(e);
    if (!combo) return;
    cleanup();
    etichetta.textContent = "binding…";
    try {
      const desc = await invoke("registra_scorciatoia", { combo });
      etichetta.textContent = desc || combo;
      debug("scorciatoia registrata: " + (desc || combo));
    } catch (err) {
      etichetta.textContent = "errore";
      debug("scorciatoia: " + err);
      setTimeout(apriImpostazioni, 1200);
    }
  };
  window.addEventListener("keydown", onKey, true);
  setTimeout(() => {
    if (etichetta.classList.contains("imp-registra-attesa")) {
      cleanup();
      etichetta.textContent = "nessuna (scaduto)";
      setTimeout(apriImpostazioni, 1200);
    }
  }, 15000);
}

// ---------------------------------------------------------------- ciclo e avvio

async function polla() {
  let st;
  try {
    st = await invoke("stato_audio");
  } catch {
    return;
  }
  const cambiata = st.inCorso !== suonando || st.inPausa !== inPausa;
  if (suonando && !st.inCorso) {
    suonando = false;
    inPausa = false;
    document.body.classList.remove("suona");
    $("mini-riemp").style.width = "0%";
    $("ca-riemp").style.width = "0%";
    if (schedaAudio) schedaAudio.classList.add("finita");
    if (!trascinando) avatar.suona("saluta");
    debug("brano finito");
  } else {
    suonando = st.inCorso;
    inPausa = st.inPausa;
    document.body.classList.toggle("suona", suonando);
    const perc = st.durata ? Math.min(100, (st.posizione / st.durata) * 100) : 0;
    $("mini-riemp").style.width = `${perc}%`;
    $("ca-riemp").style.width = `${perc}%`;
    $("ca-tempo").textContent = `${fmtTempo(st.posizione)} / ${fmtTempo(st.durata)}`;
    $("ca-gioca").innerHTML = st.inPausa ? ICONE_LETTORE.play : ICONE_LETTORE.pausa;
  }
  if (schedaAudio && schedaAudio._aggiorna) schedaAudio._aggiorna(st);
  if (cambiata) visoDiStato(); // pausa/ripresa: l'avatar si ferma o riparte
}

async function avvia() {
  avatar = new Avatar($("avatar"));
  applicaIcone();

  let nome = localStorage.getItem("daprod.personaggio") || "cassetta";
  if (!PERSONAGGI.includes(nome)) nome = "cassetta";
  try { await avatar.scegli(nome); } catch { await avatar.scegli("cassetta"); }
  avatar.suona("entra", "saluta");

  try { $("versione").textContent = await invoke("app_version"); } catch { $("versione").textContent = ""; }
  try { $("imp-versione").textContent = await invoke("app_version"); } catch {}
  debug("avvia: 1 versione ok");

  const win = getCurrentWindow();

  // ---- impostazioni: si parte sempre interattivi (l'eterea si riattiva a mano)
  try {
    impostazioni = await invoke("leggi_impostazioni");
    if (impostazioni.eterea) {
      impostazioni.eterea = false;
      await invoke("salva_impostazioni", { imp: impostazioni });
    }
    await win.setIgnoreCursorEvents(false);
    await win.setAlwaysOnTop(impostazioni.sempreSopra !== false);
  } catch (e) { debug("impostazioni iniziali: " + e); }

  // ---- × : nel pannello torna mini, in mini minimizza (mai chiudere: la musica continua)
  $("chiudi").onclick = async () => {
    if (modo === "pannello") {
      riduci();
      return;
    }
    await win.minimize();
    debug("Minimizzato: la musica continua — clic sull'icona nella barra per tornare");
  };
  $("espandi").onclick = () => (modo === "pannello" ? riduci() : espandi());
  $("cambia").onclick = async () => {
    const ora = PERSONAGGI.indexOf(avatar.nome);
    const prossimo = PERSONAGGI[(ora + 1) % PERSONAGGI.length];
    try {
      await avatar.scegli(prossimo);
      localStorage.setItem("daprod.personaggio", prossimo);
      avatar.suona("entra", "saluta");
      debug("personaggio: " + prossimo);
    } catch { /* se il foglio manca, resta dov'è */ }
  };
  $("eterea").onclick = () => applicaEterea(!eterea);
  $("impostazioni").onclick = async () => {
    await espandi();
    selezionaTab(tabAttiva === "impostazioni" ? "file" : "impostazioni");
  };

  // ---- striscia controlli audio (mini e pannello: appare avvicinandosi)
  $("ca-gioca").onclick = async () => {
    if (!suonando) { debug("niente in riproduzione"); return; }
    if (inPausa) { await invoke("riprendi"); inPausa = false; }
    else { await invoke("pausa"); inPausa = true; }
    visoDiStato();
  };
  $("ca-stop").onclick = async () => {
    await invoke("ferma");
    suonando = false;
    inPausa = false;
    document.body.classList.remove("suona");
    if (schedaAudio) schedaAudio.classList.add("finita");
    visoDiStato();
  };
  $("ca-indietro").onclick = () => saltaRelativo(-10);
  $("ca-avanti").onclick = () => saltaRelativo(10);
  $("ca-barra").onclick = async (e) => {
    try {
      const st = await invoke("stato_audio");
      if (!st.durata || !st.inCorso) return;
      const rect = $("ca-barra").getBoundingClientRect();
      const frac = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
      const np = await invoke("salta_a", { posizione: frac * st.durata });
      debug(`saltato a ${fmtTempo(np)}`);
    } catch (err) { debug(err); }
  };
  $("ca-vol").oninput = () => invoke("volume", { val: $("ca-vol").value / 100 }).catch(() => {});

  // seconda fila della striscia: scorciatoie verso il resto dell'app
  $("ca-impostazioni").onclick = async () => { await espandi(); selezionaTab("impostazioni"); };
  $("ca-eterea").onclick = () => applicaEterea(!eterea);
  $("ca-espandi").onclick = () => (modo === "pannello" ? riduci() : espandi());
  $("ca-screenshot").onclick = async () => {
    try { debug("screenshot: " + (await invoke("screenshot_area"))); }
    catch (e) { debug("screenshot: " + e); }
  };

  // ---- menu contestuale (tasto destro): le impostazioni raggiungibili anche in mini
  const menu = $("menu-contesto");
  const chiudiMenu = () => { menu.hidden = true; };
  const voce = (svgIc, testo, fn) => {
    const b = document.createElement("button");
    b.innerHTML = `<span class="voce-ic">${svgIc}</span>${testo}`;
    b.onclick = (ev) => { ev.stopPropagation(); chiudiMenu(); fn(); };
    menu.appendChild(b);
  };
  document.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    menu.innerHTML = "";
    voce(modo === "mini" ? ICONE.ingrandisci : ICONE.riduci, modo === "mini" ? "Espandi" : "Riduci", () => (modo === "mini" ? espandi() : riduci()));
    voce(ICONE.ingranaggio, "Impostazioni", async () => { await espandi(); selezionaTab("impostazioni"); });
    voce(ICONE.occhio, eterea ? "Esci dall'eterea" : "Modalità eterea", () => applicaEterea(!eterea));
    voce(ICONE.avatar, "Cambia personaggio", () => { const h = $("cambia").onclick; if (h) h(); });
    voce(ICONE_LETTORE.foto, "Screenshot ad area", async () => {
      try { debug("screenshot: " + (await invoke("screenshot_area"))); }
      catch (err) { debug("screenshot: " + err); }
    });
    voce(ICONE.riduci, "Minimizza", async () => { await getCurrentWindow().minimize(); });
    voce(ICONE.chiudi, "Esci da DaProd", async () => { try { await invoke("esci_app"); } catch { getCurrentWindow().close(); } });
    menu.hidden = false;
    const w = menu.offsetWidth || 180;
    const h = menu.offsetHeight || 220;
    menu.style.left = Math.max(2, Math.min(e.clientX, window.innerWidth - w - 4)) + "px";
    menu.style.top = Math.max(2, Math.min(e.clientY, window.innerHeight - h - 4)) + "px";
  });
  window.addEventListener("click", chiudiMenu);
  window.addEventListener("blur", chiudiMenu);

  // ---- tab
  document.querySelectorAll("#tabs button").forEach((b) => {
    b.onclick = () => selezionaTab(b.dataset.tab);
  });

  // ---- cassetti: pulsanti testata
  $("nuovo-cassetto").onclick = async () => {
    try {
      const id = await invoke("create_cassetto");
      cassettoAttivo = id;
      await caricaCassetti();
      const chip = [...document.querySelectorAll("#lista-cassetti .cassetto")].find((c) => c.classList.contains("attivo"));
      const c = cassetti.find((x) => x.id === id);
      if (chip && c) rinominaInline(chip, c);
    } catch (e) { debug(e); }
  };
  $("rinomina-cassetto").onclick = () => {
    const chip = document.querySelector("#lista-cassetti .cassetto.attivo");
    const c = cassetti.find((x) => x.id === cassettoAttivo);
    if (chip && c) rinominaInline(chip, c);
  };
  $("elimina-cassetto").onclick = async () => {
    if (!cassettoAttivo) return;
    if (cassettoAttivo === "In arrivo") { debug("«In arrivo» non si elimina"); return; }
    const c = cassetti.find((x) => x.id === cassettoAttivo);
    if (!confirm(`Eliminare il cassetto «${(c && c.label) || cassettoAttivo}»? Deve essere vuoto.`)) return;
    try {
      await invoke("delete_cassetto", { id: cassettoAttivo });
      debug("cassetto eliminato");
      cassettoAttivo = null;
      await caricaCassetti();
    } catch (e) { debug("elimina cassetto: " + e); }
  };
  $("apri-cartella").onclick = async () => {
    try {
      const p = await invoke("percorso_libreria", { drawer: cassettoAttivo });
      await invoke("apri_esterno", { percorso: p });
    } catch (e) { debug(e); }
  };

  // ---- appunti
  $("svuota-appunti").onclick = async () => {
    if (!confirm("Svuotare tutta la storia degli appunti?")) return;
    try { await invoke("svuota_appunti"); await caricaAppunti(); } catch (e) { debug(e); }
  };

  // ---- impostazioni: scorciatoia screenshot, eterea, sempre sopra, uscita
  $("imp-registra").onclick = registraScorciatoia;
  $("imp-prova").onclick = async () => {
    try {
      const out = await invoke("screenshot_area");
      debug("screenshot: " + out);
    } catch (e) { debug("screenshot: " + e); }
  };
  $("imp-togli").onclick = async () => {
    try {
      await invoke("rimuovi_scorciatoia");
      $("imp-scorciatoia-attuale").textContent = "nessuna";
      debug("scorciatoia rimossa");
    } catch (e) { debug(e); }
  };
  $("imp-eterea").onchange = () => applicaEterea($("imp-eterea").checked);
  $("imp-sopra").onchange = async () => {
    impostazioni.sempreSopra = $("imp-sopra").checked;
    try {
      await win.setAlwaysOnTop(impostazioni.sempreSopra);
      await invoke("salva_impostazioni", { imp: impostazioni });
    } catch (e) { debug(e); }
  };
  $("imp-esci").onclick = async () => {
    if (!confirm("Uscire da DaProd? La musica si fermerà.")) return;
    try { await invoke("esci_app"); } catch (e) { debug(e); win.close(); }
  };
  $("imp-associa").onclick = async () => {
    const b = $("imp-associa");
    b.disabled = true;
    try {
      debug(await invoke("associa_file"));
    } catch (e) {
      debug("associa: " + e);
    }
    b.disabled = false;
  };

  // ---- trascinamento finestra: dalla barra, e dall'avatar (con soglia).
  // Doppio clic sull'avatar in mini: espandi.
  $("barra").addEventListener("mousedown", (e) => {
    if (e.target === $("barra") || e.target.classList.contains("titolo")) win.startDragging();
  });
  let premuto = null;
  const zona = $("zona-avatar");
  zona.addEventListener("mousedown", (e) => {
    premuto = { x: e.screenX, y: e.screenY, mosso: false };
  });
  zona.addEventListener("dblclick", () => {
    if (modo === "mini") espandi();
  });
  window.addEventListener("mousemove", (e) => {
    if (premuto && !premuto.mosso && Math.abs(e.screenX - premuto.x) + Math.abs(e.screenY - premuto.y) > 6) {
      premuto.mosso = true;
      win.startDragging();
    }
  });
  window.addEventListener("mouseup", async () => {
    if (premuto && !premuto.mosso) {
      // clic semplice sull'avatar
      if (suonando) {
        if (inPausa) { await invoke("riprendi"); inPausa = false; }
        else { await invoke("pausa"); inPausa = true; }
        visoDiStato();
      } else {
        avatar.suona("saluta");
      }
    }
    premuto = null;
  });

  // ---- posizione ricordata
  try {
    const pos = JSON.parse(localStorage.getItem("daprod.posizione") || "null");
    const dpi = window.__TAURI__.dpi;
    if (pos && dpi && dpi.PhysicalPosition) {
      await win.setPosition(new dpi.PhysicalPosition(pos.x, pos.y));
    }
  } catch { /* la posizione resta quella di default */ }
  let salvaTra = null;
  try {
    await win.onMoved(({ payload }) => {
      clearTimeout(salvaTra);
      salvaTra = setTimeout(() => {
        try { localStorage.setItem("daprod.posizione", JSON.stringify({ x: payload.x, y: payload.y })); } catch {}
      }, 400);
    });
  } catch { /* se onMoved non c'è, pazienza */ }
  debug("avvia: 2 posizione ok");

  // ---- drag and drop dei file: eventi grezzi di Tauri (serve la capability
  // core:event:allow-listen, altrimenti il webview rifiuta plugin:event|listen)
  const { listen } = window.__TAURI__.event;
  const entra = () => {
    if (!trascinando) debug("drag: il file è sopra la finestra");
    trascinando = true;
    document.body.classList.add("trascina");
    avatar.suona("attento");
  };
  const esci = () => {
    trascinando = false;
    document.body.classList.remove("trascina");
    visoDiStato();
  };
  try { await listen("tauri://drag-enter", entra); } catch (e) { debug("listen enter: " + e); }
  try { await listen("tauri://drag-over", () => { trascinando = true; document.body.classList.add("trascina"); }); } catch (e) { debug("listen over: " + e); }
  try {
    await listen("tauri://drag-drop", (ev) => {
      esci();
      instrada((ev.payload && ev.payload.paths) || []);
    });
  } catch (e) { debug("listen drop: " + e); }
  try { await listen("tauri://drag-leave", esci); } catch (e) { debug("listen leave: " + e); }
  debug("avvia: 3 drag ok");

  // ---- protezione: se il webview prova ad aprire lui il file trascinato, glielo impediamo
  window.addEventListener("dragover", (e) => e.preventDefault());
  window.addEventListener("drop", (e) => e.preventDefault());

  // ---- vassoio e «Invia a DaProd»
  try {
    await listen("audio-fermo-da-tray", () => {
      suonando = false;
      inPausa = false;
      document.body.classList.remove("suona");
      if (schedaAudio) schedaAudio.classList.add("finita");
      visoDiStato();
      debug("fermato dal vassoio");
    });
  } catch (e) { debug("listen tray: " + e); }
  try {
    await listen("eterea-cambiata", (ev) => {
      eterea = !!ev.payload;
      impostazioni.eterea = eterea;
      document.body.classList.toggle("eterea", eterea);
      const chk = $("imp-eterea");
      if (chk) chk.checked = eterea;
      debug(eterea ? "eterea attivata dal vassoio" : "eterea disattivata dal vassoio");
    });
  } catch (e) { debug("listen eterea: " + e); }
  try {
    await listen("daprod-imported", async (ev) => {
      const paths = ev.payload || [];
      debug(`ricevuti ${paths.length} file in «In arrivo»`);
      for (const p of paths) {
        try {
          const info = await invoke("info_file", { percorso: p });
          if (info.tipo === "audio") await schedaAudioCrea(info);
          else { await espandi(); schedaPerTipo(info); }
        } catch (e2) { debug(e2); }
      }
      if (modo === "pannello" && tabAttiva === "cassetti") caricaCassetti();
    });
    await listen("daprod-errore", (ev) => debug("import: " + ev.payload));
  } catch (e) { debug("listen import: " + e); }

  // ---- appunti e screenshot dal Rust
  try {
    await listen("appunti-nuovo", () => {
      if (modo === "pannello" && tabAttiva === "appunti") caricaAppunti();
    });
  } catch (e) { debug("listen appunti: " + e); }
  try {
    await listen("screenshot-fatto", (ev) => debug("screenshot salvato: " + ev.payload));
    await listen("scorciatoia-errore", (ev) => debug("scorciatoia: " + ev.payload));
  } catch (e) { debug("listen screenshot: " + e); }

  // ---- autotest (DAPROD_AUTOTEST): simula un drop dopo 5 secondi
  try {
    await window.__TAURI__.event.listen("autotest", (ev) => {
      debug("autotest: " + (ev.payload || []).length + " file");
      instrada(ev.payload || []);
    });
  } catch (e) { debug("autotest non disponibile: " + e); }
  debug("avvia: 4 autotest ok");

  setInterval(polla, 300);
  applicaModo();
  debug("pronto — trascina un file sull'avatar");
}

window.addEventListener("DOMContentLoaded", avvia);
