# DaProdProduzioni

L'avatar di Cammo: un piccolo personaggio retro (musicassetta & amici) che vive sul
desktop, si espande solo quando serve e fa le cose di famiglia: **converte file**,
**organizza in cassetti**, **suona**, **edita leggero**, raccoglie gli **appunti** e
scatta **screenshot** con un tasto globale.

Nato su Linux (Tauri 2 + Rust + JS vanilla), in corsa per Windows via CI.

## Funzioni

- **Mini senza cornice**: solo l'avatar sul desktop; i controlli player appaiono al
  passaggio del mouse e spariscono subito dopo. Doppio clic: pannello grande.
- **Pannello**: tab File, Cassetti, Converti, Modifica, Appunti, Impostazioni.
  Nel pannello i controlli player NON ci sono (stanno nella mini e nel menu col
  tasto destro), per decisione di Cammo.
- **Converti**: chip dei formati, slider del peso massimo, uscita
  `nome-DaPConvertito.est` accanto all'originale (ffmpeg).
- **Cassetti**: cartelle-cassetto con miniature, rinomina, elimina.
- **Appunti**: storia dei copia-incolla come la clipboard del sistema.
- **Screenshot globale**: tasto a scelta (portale XDG GlobalShortcuts, stile ShareX:
  premi, trascini, rilasci → file in `Libreria/Schermate` + appunti).
- **Modalità eterea**: l'avatar diventa attraversabile dai clic; si esce dall'icona
  nella barra o ridando il focus alla finestra.
- **Invia a DaProd**: tasto destro in Dolphin → il file arriva all'istanza viva.
- **Associa i file**: dalle impostazioni, DaProdProduzioni diventa l'app predefinita
  per i tipi che sa aprire.

## Build

Servono Rust, Node e le dipendenze GTK/WebKitGTK (Linux).

```sh
npm install
npm run tauri build        # oppure: npx tauri build --bundles appimage
```

Su macchine senza fuse: `APPIMAGE_EXTRACT_AND_RUN=1`.

La CI (`.github/workflows/release.yml`) builda AppImage Linux e installer Windows
a ogni tag `v*`.

## Attribuzioni

- Avatar animati e icone SVG: repo [DaP-Convertitore](https://github.com/cammo22/DaP-Convertitore) di cammo22 (MIT), vedi `ui/avatar/TERZE-PARTI.md`.
- Tutto il resto: codice di questa repo, MIT.
