// Icone DaProdProduzioni — prese dal DaP-Convertitore di famiglia (MIT).
// Griglia 24×24, tratto 1.8, currentColor. Personalizzabili: ogni chiave è un SVG.
const svg = (corpo) => `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">${corpo}</svg>`;

const ICONE = {
  chiudi: svg(`<path d="M6 6l12 12M18 6L6 18"/>`),
  riduci: svg(`<path d="M6 12h12"/>`),
  ingrandisci: svg(`<rect x="6" y="6" width="12" height="12" rx="2"/>`),
  piu: svg(`<path d="M12 5v14M5 12h14"/>`),
  cartella: svg(`<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>`),
  apri: svg(`<path d="M14 4h6v6"/><path d="M20 4l-9 9"/><path d="M19 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4"/>`),
  confronta: svg(`<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M12 3v18"/><path d="M8 10l-2 2 2 2M16 10l2 2-2 2"/>`),
  ingranaggio: svg(`<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.7 1.7 0 0 0 .3 1.8l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.8-.3 1.7 1.7 0 0 0-1 1.5V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1.1-1.5 1.7 1.7 0 0 0-1.8.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1a1.7 1.7 0 0 0 .3-1.8 1.7 1.7 0 0 0-1.5-1H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.5-1.1 1.7 1.7 0 0 0-.3-1.8l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.8.3h0a1.7 1.7 0 0 0 1-1.5V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.5 1.7 1.7 0 0 0 1.8-.3l.1-.1a2 2 0 1 1 2.8 2.8l-.1.1a1.7 1.7 0 0 0-.3 1.8v0a1.7 1.7 0 0 0 1.5 1H21a2 2 0 1 1 0 4h-.1a1.7 1.7 0 0 0-1.5 1z"/>`),
  play: svg(`<path d="M7 5l12 7-12 7z" fill="currentColor"/>`),
  pausa: svg(`<rect x="6" y="5" width="4.2" height="14" rx="1" fill="currentColor" stroke="none"/><rect x="13.8" y="5" width="4.2" height="14" rx="1" fill="currentColor" stroke="none"/>`),
  ferma: svg(`<rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor"/>`),
  ok: svg(`<path d="M5 12.5l4.5 4.5L19 7.5"/>`),
  novita: svg(`<path d="M12 3l1.8 5.2L19 10l-5.2 1.8L12 17l-1.8-5.2L5 10l5.2-1.8z"/><path d="M18.5 16l.7 2 2 .7-2 .7-.7 2-.7-2-2-.7 2-.7z"/>`),
  avatar: svg(`<rect x="3" y="6" width="18" height="12" rx="2.5"/><circle cx="9" cy="12" r="2"/><circle cx="15" cy="12" r="2"/><path d="M9.5 16h5"/>`),
  attenzione: svg(`<path d="M12 3l9.5 17h-19z"/><path d="M12 10v4M12 17.5v.01"/>`),
  freccia: svg(`<path d="M5 12h14M13 6l6 6-6 6"/>`),
  indietro: svg(`<path d="M19 12H5M11 6l-6 6 6 6"/>`),
  scarica: svg(`<path d="M12 4v12M6 11l6 6 6-6M5 20h14"/>`),
  cestino: svg(`<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>`),
  fulmine: svg(`<path d="M13 2L4 14h7l-1 8 9-12h-7z" fill="currentColor" stroke="none"/>`),
  chip: svg(`<rect x="6" y="6" width="12" height="12" rx="2"/><path d="M9 2v4M15 2v4M9 18v4M15 18v4M2 9h4M2 15h4M18 9h4M18 15h4"/>`),
  aggiungi: svg(`<path d="M12 5v14M5 12h14"/>`),
  menu: svg(`<path d="M4 7h16M4 12h16M4 17h10"/>`),
  occhio: svg(`<path d="M2 12s3.6-7 10-7 10 7 10 7-3.6 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>`),
  cerca: svg(`<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>`),
  su: svg(`<path d="M6 15l6-6 6 6"/>`),
  giu: svg(`<path d="M6 9l6 6 6-6"/>`),
  pannello: svg(`<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M15 4v16"/>`),
  copia: svg(`<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>`),
};


// Set del lettore (da ui/src/lettore/comune.ts):
const ICONE_LETTORE = {
  play: svg(`<path d="M7 4.5l12.5 7.5L7 19.5z" fill="currentColor"/>`),
  pausa: svg(`<rect x="6.5" y="5" width="3.6" height="14" rx="1" fill="currentColor"/><rect x="13.9" y="5" width="3.6" height="14" rx="1" fill="currentColor"/>`),
  indietro10: svg(`<path d="M4 12a8 8 0 1 0 2.3-5.6"/><path d="M4 4v4h4"/><text x="12" y="15.2" font-size="7" font-family="Rajdhani" font-weight="700" fill="currentColor" stroke="none" text-anchor="middle">10</text>`),
  avanti10: svg(`<path d="M20 12a8 8 0 1 1-2.3-5.6"/><path d="M20 4v4h-4"/><text x="12" y="15.2" font-size="7" font-family="Rajdhani" font-weight="700" fill="currentColor" stroke="none" text-anchor="middle">10</text>`),
  volume: svg(`<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" stroke-width="1.2"/><path d="M15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11"/>`),
  muto: svg(`<path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" fill="currentColor" stroke-width="1.2"/><path d="M16 9.5l5 5M21 9.5l-5 5"/>`),
  schermo: svg(`<path d="M4 9V5h4M20 9V5h-4M4 15v4h4M20 15v4h-4"/>`),
  finestra: svg(`<path d="M9 5v4H5M15 5v4h4M9 19v-4H5M15 19v-4h4"/>`),
  sottotitoli: svg(`<rect x="3" y="5" width="18" height="14" rx="2.5"/><path d="M7 15h4M13 15h4M7 11h2M11 11h6"/>`),
  tracce: svg(`<path d="M4 7h10M4 12h16M4 17h7"/><circle cx="18" cy="7" r="2"/><circle cx="15" cy="17" r="2"/>`),
  velocita: svg(`<path d="M4.5 16a8 8 0 1 1 15 0"/><path d="M12 15l4-5"/><circle cx="12" cy="15.3" r="1.2" fill="currentColor"/>`),
  pip: svg(`<rect x="3" y="5" width="18" height="14" rx="2"/><rect x="12" y="11" width="7" height="6" rx="1" fill="currentColor"/>`),
  foto: svg(`<path d="M4 8h3l2-2.5h6L17 8h3v11H4z"/><circle cx="12" cy="13" r="3.5"/>`),
  prima: svg(`<path d="M6 5v14"/><path d="M19 5L9 12l10 7z" fill="currentColor"/>`),
  dopo: svg(`<path d="M18 5v14"/><path d="M5 5l10 7-10 7z" fill="currentColor"/>`),
  ripeti: svg(`<path d="M4 11V9a3 3 0 0 1 3-3h12l-3-3M20 13v2a3 3 0 0 1-3 3H5l3 3"/>`),
  elenco: svg(`<path d="M9 6h11M9 12h11M9 18h11"/><circle cx="4.5" cy="6" r="1" fill="currentColor"/><circle cx="4.5" cy="12" r="1" fill="currentColor"/><circle cx="4.5" cy="18" r="1" fill="currentColor"/>`),
  sx: svg(`<path d="M15 5l-7 7 7 7"/>`),
  dx: svg(`<path d="M9 5l7 7-7 7"/>`),
  zoomPiu: svg(`<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4M11 8v6M8 11h6"/>`),
  zoomMeno: svg(`<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4M8 11h6"/>`),
  adatta: svg(`<rect x="4" y="4" width="16" height="16" rx="2"/><path d="M9 4v16M15 4v16" stroke-dasharray="2 2"/>`),
  ruota: svg(`<path d="M20 12a8 8 0 1 1-2.6-5.9"/><path d="M20 4v5h-5"/>`),
  info: svg(`<circle cx="12" cy="12" r="9"/><path d="M12 11v6M12 7.5v.01"/>`),
  converti: svg(`<path d="M5 9a7 7 0 0 1 12.5-3.5L20 8"/><path d="M20 3v5h-5"/><path d="M19 15a7 7 0 0 1-12.5 3.5L4 16"/><path d="M4 21v-5h5"/>`),
  apriCon: svg(`<rect x="3" y="3" width="8" height="8" rx="2"/><rect x="13" y="3" width="8" height="8" rx="2"/><rect x="3" y="13" width="8" height="8" rx="2"/><path d="M17 14v6M14 17h6"/>`),
  cartella: svg(`<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>`),
  cestino: svg(`<path d="M4 7h16M9 7V4h6v3M6 7l1 13h10l1-13"/>`),
  tasti: svg(`<rect x="2.5" y="6" width="19" height="12" rx="2"/><path d="M6 10h.01M9 10h.01M12 10h.01M15 10h.01M18 10h.01M7 14h10"/>`),
  chiudi: svg(`<path d="M6 6l12 12M18 6L6 18"/>`),
  riduci: svg(`<path d="M6 12h12"/>`),
  ingrandisci: svg(`<rect x="6" y="6" width="12" height="12" rx="2"/>`),
  cerca: svg(`<circle cx="11" cy="11" r="7"/><path d="M20 20l-4-4"/>`),
  copia: svg(`<rect x="8" y="8" width="12" height="12" rx="2"/><path d="M16 8V6a2 2 0 0 0-2-2H6a2 2 0 0 0-2 2v8a2 2 0 0 0 2 2h2"/>`),
  sfondo: svg(`<rect x="3" y="4" width="18" height="13" rx="2"/><path d="M8 21h8M12 17v4"/><path d="M7 13l3-3 2 2 3-3 2 2"/>`),
  diapo: svg(`<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M10 8l4 2-4 2z" fill="currentColor"/><path d="M8 20h8"/>`),
  estrai: svg(`<path d="M12 3v12M7 10l5 5 5-5"/><path d="M4 17v3h16v-3"/>`),
  scarica: svg(`<path d="M12 4v12M6 11l6 6 6-6M5 20h14"/>`),
  ok: svg(`<path d="M5 12.5l4.5 4.5L19 7.5"/>`),
  avvolgi: svg(`<path d="M4 6h16M4 12h13a3 3 0 0 1 0 6h-4"/><path d="M15 16l-2 2 2 2"/><path d="M4 18h5"/>`),
  sole: svg(`<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M2 12h2M20 12h2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>`),
  griglia: svg(`<path d="M3 9h18M3 15h18M9 3v18M15 3v18"/>`),
  cubo: svg(`<path d="M12 2l9 5v10l-9 5-9-5V7z"/><path d="M12 22V12M21 7l-9 5-9-5"/>`),
  libro: svg(`<path d="M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z"/><path d="M4 19V5"/><path d="M8 7h7M8 11h5"/>`),
  codice: svg(`<path d="M8 7l-5 5 5 5M16 7l5 5-5 5M13.5 4l-3 16"/>`),
  impaginato: svg(`<path d="M6 2h8l5 5v15H6z"/><path d="M14 2v5h5"/><path d="M9 12h7M9 15h7M9 18h4"/>`),
  apri: svg(`<path d="M14 4h6v6"/><path d="M20 4l-9 9"/><path d="M19 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2h4"/>`),
};

const ICONA_CATEGORIA = {
  video: svg(`<rect x="3" y="6" width="13" height="12" rx="2"/><path d="M16 10l5-3v10l-5-3z"/>`),
  audio: svg(`<path d="M9 18V5l11-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="17" cy="16" r="3"/>`),
  immagine: svg(`<rect x="3" y="4" width="18" height="16" rx="2"/><circle cx="9" cy="10" r="2"/><path d="M21 16l-5-5-9 9"/>`),
  pdf: svg(`<path d="M6 2h8l5 5v15H6z"/><path d="M14 2v5h5"/><path d="M9 14h6M9 17h4"/>`),
  documento: svg(`<path d="M6 2h8l5 5v15H6z"/><path d="M14 2v5h5"/><path d="M9 12h7M9 15h7M9 18h5"/>`),
  foglio: svg(`<rect x="3" y="4" width="18" height="16" rx="2"/><path d="M3 10h18M3 15h18M9 4v16M15 4v16"/>`),
  presentazione: svg(`<rect x="3" y="4" width="18" height="12" rx="2"/><path d="M12 16v4M8 20h8"/><path d="M8 12l3-3 2 2 3-3"/>`),
  testo: svg(`<path d="M5 6h14M5 10h14M5 14h10M5 18h7"/>`),
  dati: svg(`<ellipse cx="12" cy="6" rx="8" ry="3"/><path d="M4 6v6c0 1.7 3.6 3 8 3s8-1.3 8-3V6"/><path d="M4 12v6c0 1.7 3.6 3 8 3s8-1.3 8-3v-6"/>`),
  sottotitoli: svg(`<rect x="3" y="5" width="18" height="14" rx="2"/><path d="M7 15h4M13 15h4M7 11h10"/>`),
  archivio: svg(`<rect x="4" y="3" width="16" height="18" rx="2"/><path d="M12 3v2M12 7v2M12 11v2"/><rect x="10" y="14" width="4" height="4" rx="1"/>`),
  cartella: svg(`<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/>`),
  altro: svg(`<path d="M6 2h8l5 5v15H6z"/><path d="M14 2v5h5"/>`),
};

const COLORE_CATEGORIA = {
  video: "#ff3df2",
  audio: "#5dffb4",
  immagine: "#35e8ff",
  pdf: "#ff4d6d",
  documento: "#6ea8ff",
  foglio: "#4dd17a",
  presentazione: "#ff9f43",
  testo: "#ebe7f4",
  dati: "#b48cff",
  sottotitoli: "#ffd54a",
  archivio: "#c9a26b",
  cartella: "#ffb23d",
  altro: "#a19db0",
};

const LOGO_SVG = `<svg viewBox="0 0 64 64"><defs>
<linearGradient id="lo" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="#fffbe0"/><stop offset=".45" stop-color="#ffd54a"/><stop offset="1" stop-color="#b87a14"/></linearGradient>
<linearGradient id="lm" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="#ff7bf6"/><stop offset="1" stop-color="#8a2cff"/></linearGradient></defs>
<circle cx="32" cy="32" r="28" fill="#150e24" stroke="url(#lo)" stroke-width="4"/>
<path d="M20 26a13 13 0 0 1 23-6" fill="none" stroke="url(#lm)" stroke-width="4" stroke-linecap="round"/><path d="M45 13v8h-8" fill="none" stroke="url(#lm)" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
<path d="M44 38a13 13 0 0 1-23 6" fill="none" stroke="url(#lo)" stroke-width="4" stroke-linecap="round"/><path d="M19 51v-8h8" fill="none" stroke="url(#lo)" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
</svg>`;
