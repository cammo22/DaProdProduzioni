//! DaProd Avatar 0.5.0 — il guscio Rust.
//!
//! Tre anime in un processo solo:
//! - avatar: riproduzione audio con rodio in un filo dedicato (OutputStream non è
//!   Send), con seek via riapertura + skip_duration
//! - libreria: cassetti, conversioni (anche a peso target), editing via ffmpeg
//! - presenza: icona nel vassoio di sistema; la riproduzione continua anche a
//!   finestra nascosta; `--import` da «Invia a DaProd» passa i file all'istanza viva
//!
//! Su WebKitGTK 2.5x + Wayland il drag and drop dei file è rotto (bug webkit
//! 320301/323277): per questo, se c'è un display X, preferiamo il backend x11.

mod library;

use library::{
    converted_name, create_unnamed_drawer, delete_inside, drain_queue, drawer_label,
    enqueue_import, ensure_library, ensure_thumb, ffprobe_duration, import_path, inside_library,
    kind_of, library_root, list_drawers, read_text, rename_drawer, run_ffmpeg, running_pid,
    sanitize_drawer, suggestions_for, thumb_data_url, unique_in_dir, write_pid, write_text, Kind,
    DEFAULT_DRAWER,
};
use rodio::Source;
use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State};

// ---------------------------------------------------------------- tipi condivisi

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Brano {
    percorso: String,
    durata: Option<f64>,
    volume: f32,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct StatoAudio {
    in_corso: bool,
    in_pausa: bool,
    posizione: f64,
    durata: Option<f64>,
    volume: f32,
    percorso: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct InfoFile {
    percorso: String,
    nome: String,
    estensione: String,
    byte: u64,
    tipo: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct TestoLetto {
    contenuto: String,
    troncato: bool,
}

#[derive(Serialize)]
struct DrawerInfo {
    id: String,
    label: String,
    count: usize,
}

#[derive(Serialize)]
struct FileInfo {
    name: String,
    path: String,
    kind: String,
    size: u64,
    suggestions: Vec<&'static str>,
    thumb: Option<String>,
    extension: String,
    snippet: Option<String>,
}

#[derive(Serialize, Deserialize, Clone)]
struct Punto {
    id: u64,
    testo: String,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct Impostazioni {
    scorciatoia_screenshot: Option<String>,
    eterea: bool,
    sempre_sopra: bool,
}

fn millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

enum Comando {
    Riproduci(String, Sender<Result<Brano, String>>),
    Pausa,
    Riprendi,
    Ferma,
    Volume(f32),
    SaltaA(f64, Sender<Result<f64, String>>),
    Stato(Sender<StatoAudio>),
}

/// Maniglia verso il filo dell'audio. `Sender` è `Send + Sync` dalla 1.72.
struct Audio(Sender<Comando>);

// ---------------------------------------------------------------- filo audio

struct Sessione {
    stream: Option<OutputStream>,
    handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    percorso: Option<String>,
    durata: Option<f64>,
    accumulata_ms: u128,
    istante: Option<Instant>,
    in_pausa: bool,
    volume: f32,
}

impl Default for Sessione {
    fn default() -> Self {
        Sessione {
            stream: None,
            handle: None,
            sink: None,
            percorso: None,
            durata: None,
            accumulata_ms: 0,
            istante: None,
            in_pausa: false,
            volume: 0.9,
        }
    }
}

fn filo_audio(rx: std::sync::mpsc::Receiver<Comando>) {
    let mut s = Sessione::default();

    for cmd in rx {
        match cmd {
            Comando::Riproduci(percorso, rispondi) => {
                let esito = (|| -> Result<Brano, String> {
                    if !Path::new(&percorso).is_file() {
                        return Err("il file non esiste".into());
                    }
                    if let Some(sink) = s.sink.take() {
                        sink.stop();
                    }
                    s.accumulata_ms = 0;
                    s.istante = None;
                    s.in_pausa = false;

                    // il dispositivo si apre una volta sola e resta vivo
                    if s.stream.is_none() || s.handle.is_none() {
                        let (stream, handle) = OutputStream::try_default()
                            .map_err(|e| format!("dispositivo audio: {e}"))?;
                        s.stream = Some(stream);
                        s.handle = Some(handle);
                    }
                    let handle = s.handle.as_ref().ok_or("handle audio mancante")?;

                    let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
                    let file = File::open(&percorso).map_err(|e| e.to_string())?;
                    let source = Decoder::new(BufReader::new(file))
                        .map_err(|e| format!("formato non supportato: {e}"))?;
                    let durata = source.total_duration().map(|d| d.as_secs_f64());

                    sink.set_volume(s.volume);
                    sink.append(source);

                    s.sink = Some(sink);
                    s.percorso = Some(percorso.clone());
                    s.durata = durata;
                    s.istante = Some(Instant::now());

                    Ok(Brano {
                        percorso,
                        durata,
                        volume: s.volume,
                    })
                })();
                let _ = rispondi.send(esito);
            }
            Comando::Pausa => {
                if !s.in_pausa {
                    if let Some(sink) = s.sink.as_ref() {
                        sink.pause();
                    }
                    if let Some(i) = s.istante.take() {
                        s.accumulata_ms += i.elapsed().as_millis();
                    }
                    s.in_pausa = true;
                }
            }
            Comando::Riprendi => {
                if s.in_pausa {
                    if let Some(sink) = s.sink.as_ref() {
                        sink.play();
                    }
                    s.istante = Some(Instant::now());
                    s.in_pausa = false;
                }
            }
            Comando::Ferma => {
                if let Some(sink) = s.sink.take() {
                    sink.stop();
                }
                s.accumulata_ms = 0;
                s.istante = None;
                s.in_pausa = false;
                s.percorso = None;
                s.durata = None;
            }
            Comando::Volume(val) => {
                let v = val.clamp(0.0, 1.0);
                s.volume = v;
                if let Some(sink) = s.sink.as_ref() {
                    sink.set_volume(v);
                }
            }
            Comando::SaltaA(target, rispondi) => {
                // rodio non sa cercare dentro un Sink già avviato: si riapre il
                // file e il decoder salta avanti (skip_duration decodifica e
                // scarta). Su file locali il costo è accettabile.
                let esito = (|| -> Result<f64, String> {
                    let percorso = s.percorso.clone().ok_or("nulla in riproduzione")?;
                    let limite = s.durata.map(|d| d - 0.05).filter(|d| *d > 0.0);
                    let nuovo = match limite {
                        Some(max) => target.max(0.0).min(max),
                        None => target.max(0.0),
                    };
                    if let Some(sink) = s.sink.take() {
                        sink.stop();
                    }
                    let handle = s.handle.as_ref().ok_or("handle audio mancante")?;
                    let sink = Sink::try_new(handle).map_err(|e| e.to_string())?;
                    let file = File::open(&percorso).map_err(|e| e.to_string())?;
                    let source = Decoder::new(BufReader::new(file))
                        .map_err(|e| e.to_string())?
                        .skip_duration(Duration::from_secs_f64(nuovo));
                    sink.set_volume(s.volume);
                    sink.append(source);
                    if s.in_pausa {
                        sink.pause();
                        s.istante = None;
                    } else {
                        s.istante = Some(Instant::now());
                    }
                    s.sink = Some(sink);
                    s.accumulata_ms = (nuovo * 1000.0) as u128;
                    Ok(nuovo)
                })();
                let _ = rispondi.send(esito);
            }
            Comando::Stato(rispondi) => {
                let coda_vuota = match s.sink.as_ref() {
                    Some(sink) => sink.empty(),
                    None => true,
                };
                let in_corso = !coda_vuota && s.percorso.is_some();
                let mut posizione = s.accumulata_ms as f64 / 1000.0;
                if in_corso && !s.in_pausa {
                    if let Some(i) = s.istante {
                        posizione += i.elapsed().as_secs_f64();
                    }
                }
                let _ = rispondi.send(StatoAudio {
                    in_corso,
                    in_pausa: s.in_pausa && in_corso,
                    posizione,
                    durata: s.durata,
                    volume: s.volume,
                    percorso: s.percorso.clone(),
                });
            }
        }
    }
}

fn chiedi<T>(audio: &Audio, cmd: Comando, rx: std::sync::mpsc::Receiver<T>) -> Result<T, String> {
    audio
        .0
        .send(cmd)
        .map_err(|_| "il filo audio non risponde più")?;
    rx.recv()
        .map_err(|_| "nessuna risposta dal filo audio".into())
}

// ---------------------------------------------------------------- comandi: audio

#[tauri::command]
fn riproduci(percorso: String, audio: State<Audio>) -> Result<Brano, String> {
    let (tx, rx) = channel();
    chiedi(&audio, Comando::Riproduci(percorso, tx), rx)?
}

#[tauri::command]
fn pausa(audio: State<Audio>) -> Result<(), String> {
    audio.0.send(Comando::Pausa).map_err(|e| e.to_string())
}

#[tauri::command]
fn riprendi(audio: State<Audio>) -> Result<(), String> {
    audio.0.send(Comando::Riprendi).map_err(|e| e.to_string())
}

#[tauri::command]
fn ferma(audio: State<Audio>) -> Result<(), String> {
    audio.0.send(Comando::Ferma).map_err(|e| e.to_string())
}

#[tauri::command]
fn volume(val: f32, audio: State<Audio>) -> Result<(), String> {
    audio.0.send(Comando::Volume(val)).map_err(|e| e.to_string())
}

#[tauri::command]
fn salta_a(posizione: f64, audio: State<Audio>) -> Result<f64, String> {
    let (tx, rx) = channel();
    chiedi(&audio, Comando::SaltaA(posizione, tx), rx)?
}

#[tauri::command]
fn stato_audio(audio: State<Audio>) -> Result<StatoAudio, String> {
    let (tx, rx) = channel();
    chiedi(&audio, Comando::Stato(tx), rx)
}

// ---------------------------------------------------------------- comandi: file

fn estensione_di(percorso: &str) -> String {
    Path::new(percorso)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn tipo_da_estensione(est: &str) -> &'static str {
    match est {
        "mp3" | "wav" | "ogg" | "oga" | "flac" | "m4a" | "aac" | "aiff" | "aif" => "audio",
        "mp4" | "webm" | "mkv" | "avi" | "mov" | "m4v" | "mpg" | "mpeg" => "video",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "svg" | "avif" => "immagine",
        "pdf" => "pdf",
        "txt" | "md" | "json" | "csv" | "log" | "xml" | "yml" | "yaml" | "toml" | "ini"
        | "conf" | "sh" | "py" | "rs" | "js" | "ts" | "css" | "html" | "htm" | "srt"
        | "vtt" => "testo",
        _ => "altro",
    }
}

#[tauri::command]
fn info_file(percorso: String) -> Result<InfoFile, String> {
    let meta = std::fs::metadata(&percorso).map_err(|e| e.to_string())?;
    if !meta.is_file() {
        return Err("non è un file".into());
    }
    let est = estensione_di(&percorso);
    let nome = Path::new(&percorso)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| percorso.clone());
    Ok(InfoFile {
        percorso: percorso.clone(),
        nome,
        estensione: est.clone(),
        byte: meta.len(),
        tipo: tipo_da_estensione(&est).to_string(),
    })
}

#[tauri::command]
fn leggi_testo(percorso: String, limite_kb: Option<u64>) -> Result<TestoLetto, String> {
    let limite = limite_kb.unwrap_or(256).clamp(1, 4096) * 1024;
    let mut f = File::open(&percorso).map_err(|e| e.to_string())?;
    let mut buf = Vec::new();
    f.by_ref()
        .take(limite + 1)
        .read_to_end(&mut buf)
        .map_err(|e| e.to_string())?;
    let troncato = buf.len() as u64 > limite;
    if troncato {
        buf.truncate(limite as usize);
    }
    Ok(TestoLetto {
        contenuto: String::from_utf8_lossy(&buf).to_string(),
        troncato,
    })
}

#[tauri::command]
fn apri_esterno(percorso: String) -> Result<(), String> {
    if !Path::new(&percorso).exists() {
        return Err("il file non esiste".into());
    }
    #[cfg(target_os = "linux")]
    let esito = std::process::Command::new("xdg-open").arg(&percorso).spawn();
    #[cfg(target_os = "windows")]
    let esito = std::process::Command::new("cmd")
        .args(["/C", "start", "", &percorso])
        .spawn();
    #[cfg(target_os = "macos")]
    let esito = std::process::Command::new("open").arg(&percorso).spawn();
    esito.map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
fn app_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

#[tauri::command]
fn traccia_debug(msg: String) -> Result<(), String> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/daprod-debug.log")
        .map_err(|e| e.to_string())?;
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    writeln!(f, "[{stamp}] {msg}").map_err(|e| e.to_string())?;
    Ok(())
}

// ---------------------------------------------------------------- comandi: cassetti

#[tauri::command]
fn list_cassetti() -> Result<Vec<DrawerInfo>, String> {
    let root = library_root();
    ensure_library(&root).map_err(|e| e.to_string())?;
    let names = list_drawers(&root).map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for id in names {
        let count = std::fs::read_dir(root.join(&id))
            .map(|rd| rd.filter_map(|e| e.ok()).count())
            .unwrap_or(0);
        out.push(DrawerInfo {
            label: drawer_label(&root, &id),
            id,
            count,
        });
    }
    Ok(out)
}

#[tauri::command]
fn create_cassetto() -> Result<String, String> {
    let root = library_root();
    create_unnamed_drawer(&root)
}

#[tauri::command]
fn rename_cassetto(id: String, label: String) -> Result<(), String> {
    let root = library_root();
    rename_drawer(&root, &id, &label)
}

#[tauri::command]
fn delete_cassetto(id: String) -> Result<(), String> {
    let root = library_root();
    let id = sanitize_drawer(&id)?;
    if id == DEFAULT_DRAWER {
        return Err("«In arrivo» non si elimina".into());
    }
    let dir = root.join(&id);
    if !dir.is_dir() {
        return Err("cassetto inesistente".into());
    }
    let n = std::fs::read_dir(&dir).map_err(|e| e.to_string())?.count();
    if n > 0 {
        return Err("il cassetto non è vuoto: sposta o elimina prima i file".into());
    }
    std::fs::remove_dir(&dir).map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_file(path: String) -> Result<(), String> {
    let root = library_root();
    delete_inside(&root, Path::new(&path))
}

#[tauri::command]
fn import_paths(paths: Vec<String>, drawer: Option<String>) -> Result<Vec<String>, String> {
    let root = library_root();
    ensure_library(&root).map_err(|e| e.to_string())?;
    let drawer = drawer.unwrap_or_else(|| DEFAULT_DRAWER.to_string());
    let mut saved = Vec::new();
    for path in paths {
        let dest = import_path(&root, &drawer, Path::new(&path))?;
        let _ = ensure_thumb(&root, &dest);
        saved.push(dest.to_string_lossy().to_string());
    }
    Ok(saved)
}

#[tauri::command]
fn percorso_libreria(drawer: Option<String>) -> Result<String, String> {
    let root = library_root();
    ensure_library(&root).map_err(|e| e.to_string())?;
    match drawer {
        Some(d) => {
            let d = sanitize_drawer(&d)?;
            let dir = root.join(&d);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            Ok(dir.to_string_lossy().to_string())
        }
        None => Ok(root.to_string_lossy().to_string()),
    }
}

fn kind_name(kind: Kind) -> &'static str {
    match kind {
        Kind::Image => "image",
        Kind::Video => "video",
        Kind::Audio => "audio",
        Kind::Text => "text",
        Kind::Other => "other",
    }
}

#[tauri::command]
fn list_files(drawer: String) -> Result<Vec<FileInfo>, String> {
    let root = library_root();
    let name = sanitize_drawer(&drawer)?;
    let dir = root.join(&name);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut files = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let path = entry.path();
        if path.file_name().and_then(|n| n.to_str()) == Some(".daprod") {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let kind = kind_of(&path);
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_uppercase();
        let snippet = if kind == Kind::Text {
            read_text(&path).ok().map(|t| t.chars().take(180).collect())
        } else {
            None
        };
        files.push(FileInfo {
            name: entry.file_name().to_string_lossy().to_string(),
            path: path.to_string_lossy().to_string(),
            kind: kind_name(kind).into(),
            size,
            suggestions: suggestions_for(&path),
            thumb: thumb_data_url(&root, &path),
            extension,
            snippet,
        });
    }
    Ok(files)
}

// ---------------------------------------------------------------- comandi: conversioni e editing

fn sibling_output(source: &Path, format: &str) -> Result<PathBuf, String> {
    let parent = source.parent().ok_or("percorso senza cartella")?;
    let name = converted_name(source, format);
    Ok(unique_in_dir(parent, &name))
}

/// Le conversioni si chiamano «nome-DaPConvertito.est» e restano accanto
/// all'originale, come chiesto da Cammo.
fn destinazione_convertita(source: &Path, format: &str) -> Result<PathBuf, String> {
    let parent = source.parent().ok_or("percorso senza cartella")?;
    let stem = source.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let name = format!("{stem}-DaPConvertito.{format}");
    Ok(unique_in_dir(parent, &name))
}

/// Immagine a peso target: jpg/webp abbassano la qualità a passi finché stanno
/// sotto il target; png rimpicciolisce le dimensioni a passi.
fn converti_immagine_peso(
    input: &str,
    output: &str,
    format: &str,
    target_kb: u64,
) -> Result<(), String> {
    let sotto = |p: &Path| -> bool {
        std::fs::metadata(p)
            .map(|m| m.len() / 1024 <= target_kb)
            .unwrap_or(false)
    };
    if format == "png" {
        for perc in [100u32, 80, 64, 50, 40, 30] {
            if perc == 100 {
                run_ffmpeg(&["-y", "-i", input, output])?;
            } else {
                let vf = format!("scale=trunc(iw*{perc}/100):trunc(ih*{perc}/100)");
                run_ffmpeg(&["-y", "-i", input, "-vf", &vf, output])?;
            }
            if sotto(Path::new(output)) {
                return Ok(());
            }
        }
        Ok(())
    } else {
        for q in [85u32, 70, 55, 40, 28, 18] {
            let qs = q.to_string();
            if format == "webp" {
                run_ffmpeg(&["-y", "-i", input, "-quality", &qs, output])?;
            } else {
                run_ffmpeg(&["-y", "-i", input, "-q:v", &qs, output])?;
            }
            if sotto(Path::new(output)) {
                return Ok(());
            }
        }
        Ok(())
    }
}

#[tauri::command]
fn convert_file(path: String, format: String, target_kb: Option<u64>) -> Result<String, String> {
    let source = PathBuf::from(&path);
    if !source.is_file() {
        return Err("il file non esiste".into());
    }
    let format = format.to_ascii_lowercase();
    let kind = kind_of(&source);
    let permesso = match kind {
        Kind::Image => matches!(format.as_str(), "png" | "jpg" | "jpeg" | "webp"),
        // da video si può anche estrarre l'audio
        Kind::Video => matches!(format.as_str(), "mp4" | "webm" | "mp3" | "wav"),
        Kind::Audio => matches!(format.as_str(), "mp3" | "wav" | "flac" | "m4a" | "ogg"),
        _ => false,
    };
    if !permesso {
        return Err(format!("conversione in {format} non supportata per questo file").into());
    }
    let dest = destinazione_convertita(&source, &format)?;
    let input = source.to_string_lossy().to_string();
    let output = dest.to_string_lossy().to_string();
    let target = target_kb.filter(|t| *t > 0);

    match (kind, target) {
        (Kind::Image, Some(t)) => converti_immagine_peso(&input, &output, &format, t)?,
        (Kind::Video, Some(t)) => {
            // bitrate video = (peso target in kbit / durata) - 128k per l'audio
            let dur = ffprobe_duration(&source).ok_or("durata non letta (ffprobe manca?)")?;
            let video_k = ((t as f64) * 8.0 / dur - 128.0).max(150.0);
            let vb = format!("{}k", video_k as u64);
            let maxrate = format!("{}k", (video_k * 1.45) as u64);
            let bufsize = format!("{}k", (video_k * 2.0) as u64);
            run_ffmpeg(&[
                "-y", "-i", &input, "-c:v", "libx264", "-b:v", &vb, "-maxrate", &maxrate,
                "-bufsize", &bufsize, "-c:a", "aac", "-b:a", "128k", &output,
            ])?;
        }
        (Kind::Audio, Some(t)) => {
            if format == "wav" || format == "flac" {
                // lossless: il peso non si comanda, vince il contenuto
                run_ffmpeg(&["-y", "-i", &input, &output])?;
            } else {
                let dur = ffprobe_duration(&source).ok_or("durata non letta (ffprobe manca?)")?;
                let abr = (((t as f64) * 8.0 / dur).round()).clamp(24.0, 320.0) as u64;
                let ba = format!("{abr}k");
                let codec = if format == "mp3" { "libmp3lame" } else { "aac" };
                run_ffmpeg(&["-y", "-i", &input, "-c:a", codec, "-b:a", &ba, &output])?;
            }
        }
        _ => run_ffmpeg(&["-y", "-i", &input, &output])?,
    }
    Ok(output)
}

#[tauri::command]
fn rotate_image(path: String) -> Result<String, String> {
    let source = PathBuf::from(&path);
    if !source.is_file() {
        return Err("il file non esiste".into());
    }
    if kind_of(&source) != Kind::Image {
        return Err("si ruotano solo le immagini".into());
    }
    let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("png");
    let dest = sibling_output(&source, ext)?;
    let input = source.to_string_lossy().to_string();
    let output = dest.to_string_lossy().to_string();
    run_ffmpeg(&["-y", "-i", &input, "-vf", "transpose=1", &output])?;
    Ok(output)
}

#[tauri::command]
fn trim_media(path: String, start: f64, end: f64) -> Result<String, String> {
    let source = PathBuf::from(&path);
    if !source.is_file() {
        return Err("il file non esiste".into());
    }
    if !(start >= 0.0) || end <= start {
        return Err("servono inizio >= 0 e fine dopo l'inizio".into());
    }
    let kind = kind_of(&source);
    let (dest, codec): (PathBuf, &[&str]) = match kind {
        Kind::Video => (
            sibling_output(&source, "mp4")?,
            &["-c:v", "libx264", "-c:a", "aac"],
        ),
        Kind::Audio => (sibling_output(&source, "m4a")?, &["-c:a", "aac"]),
        _ => return Err("si tagliano solo audio e video".into()),
    };
    let input = source.to_string_lossy().to_string();
    let output = dest.to_string_lossy().to_string();
    let start_s = format!("{start}");
    let end_s = format!("{end}");
    let mut args: Vec<String> = vec![
        "-y".into(),
        "-ss".into(),
        start_s,
        "-to".into(),
        end_s,
        "-i".into(),
        input,
    ];
    args.extend(codec.iter().map(|s| s.to_string()));
    args.push(output);
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    run_ffmpeg(&refs)?;
    Ok(dest.to_string_lossy().to_string())
}

#[tauri::command]
fn read_testo(path: String) -> Result<String, String> {
    read_text(&PathBuf::from(path))
}

#[tauri::command]
fn write_testo(path: String, text: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err("il file non esiste".into());
    }
    write_text(&p, &text)
}

#[tauri::command]
fn elimina_se_in_libreria(path: String) -> Result<(), String> {
    let root = library_root();
    if !inside_library(&root, Path::new(&path)) {
        return Err("si eliminano solo i file dentro la libreria".into());
    }
    delete_inside(&root, Path::new(&path))
}

// ---------------------------------------------------------------- appunti (storia dei copia)

#[derive(Default)]
struct StatoAppunti {
    lista: Vec<Punto>,
    ultimo: Option<String>,
}

#[derive(Clone)]
struct RegistroAppunti(Arc<Mutex<StatoAppunti>>);

fn appunti_path() -> PathBuf {
    library_root().join(".daprod").join("appunti.json")
}

fn carica_appunti() -> Vec<Punto> {
    std::fs::read_to_string(appunti_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn salva_appunti(lista: &[Punto]) {
    let p = appunti_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(t) = serde_json::to_string_pretty(lista) {
        let _ = std::fs::write(p, t);
    }
}

/// Aggiunge un testo appena copiato: i duplicati risalgono in cima come fa Klipper.
fn aggiungi_appunto(registro: &RegistroAppunti, app: &AppHandle, testo: String) {
    if testo.trim().is_empty() || testo.chars().count() > 20_000 {
        return;
    }
    let mut g = registro.0.lock().unwrap_or_else(|e| e.into_inner());
    if g.ultimo.as_deref() == Some(testo.as_str()) {
        return;
    }
    g.ultimo = Some(testo.clone());
    let punto = Punto {
        id: millis(),
        testo: testo.clone(),
    };
    g.lista.retain(|p| p.testo != testo);
    g.lista.insert(0, punto.clone());
    g.lista.truncate(100);
    salva_appunti(&g.lista);
    drop(g);
    let _ = app.emit("appunti-nuovo", punto);
}

#[cfg(target_os = "linux")]
fn avvia_raccolta_appunti(app: AppHandle, registro: RegistroAppunti) {
    // La clipboard GTK vive nel filo principale: un timer glib legge il testo
    // ogni 700 ms. Su XWayland la selezione CLIPBOARD è sincronizzata con gli
    // appunti Wayland (Klipper), quindi prendiamo anche i copia fatti altrove.
    let _ = gtk::glib::timeout_add_local(Duration::from_millis(700), move || {
        let cb = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        if let Some(testo) = cb.wait_for_text() {
            aggiungi_appunto(&registro, &app, testo.to_string());
        }
        gtk::glib::ControlFlow::Continue
    });
}

#[tauri::command]
fn list_appunti(registro: State<RegistroAppunti>) -> Result<Vec<Punto>, String> {
    let g = registro.0.lock().unwrap_or_else(|e| e.into_inner());
    Ok(g.lista.clone())
}

#[tauri::command]
fn elimina_appunto(id: u64, registro: State<RegistroAppunti>) -> Result<(), String> {
    let mut g = registro.0.lock().unwrap_or_else(|e| e.into_inner());
    g.lista.retain(|p| p.id != id);
    salva_appunti(&g.lista);
    Ok(())
}

#[tauri::command]
fn svuota_appunti(registro: State<RegistroAppunti>) -> Result<(), String> {
    let mut g = registro.0.lock().unwrap_or_else(|e| e.into_inner());
    g.lista.clear();
    g.ultimo = None;
    salva_appunti(&g.lista);
    Ok(())
}

#[tauri::command]
#[cfg(target_os = "linux")]
fn copia_appunto(app: AppHandle, testo: String) -> Result<(), String> {
    app.run_on_main_thread(move || {
        let cb = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
        cb.set_text(&testo);
        cb.store();
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
fn copia_appunto(_app: AppHandle, _testo: String) -> Result<(), String> {
    Err("appunti: per ora solo Linux".into())
}

// ---------------------------------------------------------------- impostazioni

fn impostazioni_path() -> PathBuf {
    library_root().join(".daprod").join("impostazioni.json")
}

#[tauri::command]
fn leggi_impostazioni() -> Impostazioni {
    std::fs::read_to_string(impostazioni_path())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_else(|| Impostazioni {
            sempre_sopra: true,
            ..Default::default()
        })
}

#[tauri::command]
fn salva_impostazioni(imp: Impostazioni) -> Result<(), String> {
    let p = impostazioni_path();
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let t = serde_json::to_string_pretty(&imp).map_err(|e| e.to_string())?;
    std::fs::write(p, t).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------- screenshot

#[cfg(target_os = "linux")]
fn copy_image_to_clipboard(path: &Path) -> Result<(), String> {
    let pixbuf = gdk_pixbuf::Pixbuf::from_file(path).map_err(|e| e.to_string())?;
    let cb = gtk::Clipboard::get(&gtk::gdk::SELECTION_CLIPBOARD);
    cb.set_image(&pixbuf);
    cb.store();
    Ok(())
}

/// Spectacle sceglie l'area (dialogo KDE), salva in Libreria/Schermate e
/// mette l'immagine anche negli appunti.
#[cfg(target_os = "linux")]
fn fai_screenshot(app: AppHandle) -> Result<String, String> {
    let root = library_root();
    let dir = root.join("Schermate");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let dest = unique_in_dir(&dir, &format!("Schermata-{stamp}.png"));
    let output = dest.to_string_lossy().to_string();
    let status = std::process::Command::new("/usr/sbin/spectacle")
        .args(["-r", "-b", "-n", "-k", "-o", &output])
        .status()
        .map_err(|e| format!("spectacle non parte: {e}"))?;
    if !status.success() || !dest.is_file() {
        return Err("screenshot annullato".into());
    }
    let copia = dest.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = copy_image_to_clipboard(&copia);
    });
    let _ = app.emit("screenshot-fatto", output.clone());
    Ok(output)
}

#[tauri::command]
#[cfg(target_os = "linux")]
fn screenshot_area(app: AppHandle) -> Result<String, String> {
    fai_screenshot(app)
}

/// Mette un'immagine (per es. quella uscita dall'editor) negli appunti.
#[tauri::command]
#[cfg(target_os = "linux")]
fn copia_immagine_appunti(path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    if !p.is_file() {
        return Err("il file non esiste".into());
    }
    copy_image_to_clipboard(&p)
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
fn copia_immagine_appunti(_path: String) -> Result<(), String> {
    Err("appunti immagini: per ora solo Linux".into())
}

/// Salva byte (PNG uscito dal canvas) dentro la libreria: mai fuori.
#[tauri::command]
fn salva_binario(path: String, bytes: Vec<u8>) -> Result<String, String> {
    let root = library_root();
    let p = PathBuf::from(&path);
    if !inside_library(&root, &p) {
        return Err("si salva solo dentro la libreria DaProd".into());
    }
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&p, bytes).map_err(|e| e.to_string())?;
    Ok(p.to_string_lossy().to_string())
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
fn screenshot_area(_app: AppHandle) -> Result<String, String> {
    Err("screenshot: per ora solo Linux".into())
}

#[tauri::command]
fn esci_app(app: AppHandle) {
    app.exit(0);
}

// ---------------------------------------------------------------- associazione file e id portale

const APP_ID: &str = "com.daprodproduzioni.avatar";

/// I tipi di file che DaProdProduzioni sa aprire (lista ripresa dal
/// Convertitore di famiglia, vedi wiki). Servono al .desktop e a xdg-mime.
const MIME_GESTITI: &[&str] = &[
    // audio (lista di famiglia dal DaP-Convertitore, mappata sui MIME freedesktop)
    "audio/mpeg", "audio/wav", "audio/x-wav", "audio/ogg", "audio/flac", "audio/mp4", "audio/aac",
    "audio/x-aiff", "audio/x-ms-wma", "audio/opus", "audio/x-matroska", "audio/webm",
    // video
    "video/mp4", "video/x-matroska", "video/webm", "video/quicktime", "video/x-msvideo",
    "video/x-ms-wmv", "video/x-flv", "video/mpeg", "video/3gpp", "video/ogg",
    // immagini
    "image/png", "image/jpeg", "image/webp", "image/gif", "image/bmp", "image/svg+xml",
    "image/avif", "image/tiff", "image/heic", "image/heif", "image/x-icon", "image/apng",
    // testo e documenti
    "text/plain", "text/markdown", "text/html", "text/csv", "application/pdf",
    "application/json", "application/xml", "application/rtf",
    "application/vnd.oasis.opendocument.text",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/vnd.oasis.opendocument.spreadsheet",
    "application/vnd.openxmlformats-officedocument.presentationml.presentation",
    "application/vnd.oasis.opendocument.presentation",
];

fn desktop_file_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/home/cammo".into());
    PathBuf::from(home)
        .join(".local/share/applications")
        .join(format!("{APP_ID}.desktop"))
}

/// Scrive (una volta sola) il .desktop installato: serve a Dolphin per
/// «apri con», a xdg-mime per l'associazione predefinita e al portale XDG per
/// riconoscere l'app id.
#[cfg(target_os = "linux")]
fn scrivi_desktop_file() -> Result<PathBuf, String> {
    let p = desktop_file_path();
    if p.is_file() {
        return Ok(p);
    }
    if let Some(parent) = p.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let wrapper = "/home/cammo/Scrivania/DaProd/daprod-invia.sh";
    let mimes = MIME_GESTITI.join(";");
    let corpo = format!(
        "[Desktop Entry]\n\
         Type=Application\n\
         Name=DaProdProduzioni\n\
         GenericName=Converter e player avatar\n\
         Comment=L'avatar di Cammo: converte, suona, organizza\n\
         Exec={wrapper} %F\n\
         Icon=daprodproduzioni\n\
         Terminal=false\n\
         Categories=AudioVideo;Graphics;Utility;FileTools;\n\
         MimeType={mimes};\n\
         StartupWMClass=DaProdProduzioni\n"
    );
    std::fs::write(&p, corpo).map_err(|e| e.to_string())?;
    let _ = std::process::Command::new("chmod")
        .args(["+x", &p.to_string_lossy()])
        .status();
    Ok(p)
}

/// Il portale XDG ≥ 1.21 rifiuta le app host senza id («An app id is
/// required»): l'id va dichiarato con org.freedesktop.host.portal.Registry.
/// Register(app_id) SULLA STESSA connessione bus che poi apre la sessione, e
/// deve esistere un .desktop installato con quel nome.
#[cfg(target_os = "linux")]
async fn lega_scorciatoia(
    app: &AppHandle,
    corrente: &mut Option<tokio::task::JoinHandle<()>>,
    combo: String,
) -> Result<String, String> {
    use ashpd::zbus::zvariant::{OwnedObjectPath, Value};
    use futures_util::StreamExt;
    use std::collections::HashMap;

    scrivi_desktop_file()?;
    let conn = ashpd::zbus::Connection::session()
        .await
        .map_err(|e| e.to_string())?;

    // 0. dichiara l'app id su QUESTA connessione
    let reg = ashpd::zbus::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.host.portal.Registry",
    )
    .await
    .map_err(|e| e.to_string())?;
    let zero: HashMap<&str, Value<'_>> = HashMap::new();
    let esito_reg: Result<(), ashpd::zbus::Error> = reg.call("Register", &(APP_ID, zero)).await;
    if let Err(e) = esito_reg {
        eprintln!("Registry.Register({APP_ID}): {e}");
    }

    let gs = ashpd::zbus::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.GlobalShortcuts",
    )
    .await
    .map_err(|e| e.to_string())?;

    let unico = conn
        .unique_name()
        .ok_or("nome bus mancante")?
        .as_str()
        .trim_start_matches(':')
        .replace('.', "_");

    // 1. CreateSession: richiesta + risposta sul cammino request nostro
    let token1 = format!("dpk{}", millis());
    let rp1 = format!("/org/freedesktop/portal/desktop/request/{unico}/{token1}");
    let req1 = ashpd::zbus::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        rp1.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut resp1 = req1
        .receive_signal("Response")
        .await
        .map_err(|e| e.to_string())?;
    let mut opts1: HashMap<&str, Value<'_>> = HashMap::new();
    opts1.insert("handle_token", Value::from(token1.as_str()));
    opts1.insert("session_handle_token", Value::from(format!("dps{}", millis())));
    let _: OwnedObjectPath = gs
        .call("CreateSession", &(opts1,))
        .await
        .map_err(|e| format!("CreateSession: {e}"))?;
    let msg1 = resp1.next().await.ok_or("risposta CreateSession mancante")?;
    let corpo1 = msg1.body();
    let (codice, risultati): (u32, HashMap<String, Value<'_>>) =
        corpo1.deserialize().map_err(|e| e.to_string())?;
    if codice != 0 {
        return Err(format!("CreateSession rifiutata dal portale ({codice})"));
    }
    eprintln!("SCOR-DEBUG risultati CreateSession: {risultati:?}");
    let sessione: OwnedObjectPath = match risultati.get("session_handle") {
        Some(Value::ObjectPath(p)) => p.to_owned().into(),
        // il portale KDE lo manda come stringa, non come object path
        Some(Value::Str(s)) => OwnedObjectPath::try_from(s.as_str().to_string())
            .map_err(|e| format!("session_handle strano: {e}"))?,
        _ => return Err("session_handle mancante nella risposta".into()),
    };

    // 2. BindShortcuts con la combinazione scelta da Cammo
    let token2 = format!("dpb{}", millis());
    let rp2 = format!("/org/freedesktop/portal/desktop/request/{unico}/{token2}");
    let req2 = ashpd::zbus::Proxy::new(
        &conn,
        "org.freedesktop.portal.Desktop",
        rp2.as_str(),
        "org.freedesktop.portal.Request",
    )
    .await
    .map_err(|e| e.to_string())?;
    let mut resp2 = req2
        .receive_signal("Response")
        .await
        .map_err(|e| e.to_string())?;
    let mut info: HashMap<&str, Value<'_>> = HashMap::new();
    info.insert("description", Value::from("Screenshot DaProd"));
    info.insert("preferred_trigger", Value::from(combo.as_str()));
    let lista: Vec<(&str, HashMap<&str, Value<'_>>)> = vec![(SC_ID, info)];
    let mut opts2: HashMap<&str, Value<'_>> = HashMap::new();
    opts2.insert("handle_token", Value::from(token2.as_str()));
    let _: OwnedObjectPath = gs
        .call("BindShortcuts", &(sessione.clone(), lista.as_slice(), "", opts2))
        .await
        .map_err(|e| format!("BindShortcuts: {e}"))?;
    let msg2 = resp2.next().await.ok_or("risposta BindShortcuts mancante")?;
    let (codice2, risultati2): (u32, HashMap<String, Value<'_>>) =
        msg2.body().deserialize().map_err(|e| e.to_string())?;
    if codice2 != 0 {
        return Err("bind rifiutato: combinazione occupata o permesso negato".into());
    }
    // l'etichetta mostrata resta la combinazione scelta: parla chiaro
    let desc = combo.clone();

    // 3. ascolto delle attivazioni: la connessione resta viva nel task
    if let Some(t) = corrente.take() {
        t.abort();
    }
    let app2 = app.clone();
    let h = tokio::spawn(async move {
        let Ok(mut stream) = gs.receive_signal("Activated").await else {
            return;
        };
        while let Some(msg) = stream.next().await {
            let Ok((_s, id, _t, _o)) = msg
                .body()
                .deserialize::<(OwnedObjectPath, String, u64, HashMap<String, Value<'_>>)>()
            else {
                continue;
            };
            if id == SC_ID {
                let a = app2.clone();
                // spectacle blocca finché l'utente sceglie l'area: fuori dal runtime
                std::thread::spawn(move || {
                    let _ = fai_screenshot(a);
                });
            }
        }
        drop(conn);
    });
    *corrente = Some(h);

    let mut imp = leggi_impostazioni();
    imp.scorciatoia_screenshot = Some(combo);
    let _ = salva_impostazioni(imp);
    Ok(desc)
}

/// Rende DaProdProduzioni l'app predefinita per i tipi che sa aprire.
#[tauri::command]
#[cfg(target_os = "linux")]
fn associa_file() -> Result<String, String> {
    let p = scrivi_desktop_file()?;
    let nome = p
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_string();
    if let Some(dir) = p.parent() {
        let _ = std::process::Command::new("update-desktop-database")
            .arg(dir.to_string_lossy().to_string())
            .status();
    }
    let mut args: Vec<String> = vec!["default".into(), nome];
    args.extend(MIME_GESTITI.iter().map(|s| s.to_string()));
    let stato = std::process::Command::new("xdg-mime")
        .args(&args)
        .status()
        .map_err(|e| format!("xdg-mime non parte: {e}"))?;
    if stato.success() {
        Ok(format!(
            "DaProdProduzioni è ora l'app predefinita per {} tipi di file",
            MIME_GESTITI.len()
        ))
    } else {
        Err("xdg-mime ha rifiutato l'associazione".into())
    }
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
fn associa_file() -> Result<String, String> {
    Err("associazione file: per ora solo Linux".into())
}

// ---------------------------------------------------------------- scorciatoia globale

const SC_ID: &str = "screenshot-daprod";

enum CmdScor {
    Registra(String, Sender<Result<String, String>>),
    Rimuovi(Sender<Result<(), String>>),
}

struct Scorciatoie(tokio::sync::mpsc::UnboundedSender<CmdScor>);

/// La scorciatoia globale vive nel portale XDG GlobalShortcuts (su KDE Wayland
/// è l'unica via affidabile). Il binding e l'ascolto girano in un filo con un
/// runtime tokio dedicato.
#[cfg(target_os = "linux")]
fn filo_scorciatoie(app: AppHandle, rx: tokio::sync::mpsc::UnboundedReceiver<CmdScor>) {
    std::thread::Builder::new()
        .name("scorciatoie".into())
        .spawn(move || {
            let Ok(rt) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            else {
                return;
            };
            rt.block_on(gestore_scorciatoie(app, rx));
        })
        .expect("impossibile avviare il filo scorciatoie");
}

#[cfg(target_os = "linux")]
async fn gestore_scorciatoie(
    app: AppHandle,
    mut rx: tokio::sync::mpsc::UnboundedReceiver<CmdScor>,
) {
    let mut corrente: Option<tokio::task::JoinHandle<()>> = None;

    // ripristina la scorciatoia salvata, se c'è
    let imp = leggi_impostazioni();
    if let Some(combo) = imp.scorciatoia_screenshot.clone() {
        if let Err(e) = lega_scorciatoia(&app, &mut corrente, combo).await {
            let _ = app.emit("scorciatoia-errore", format!("ripristino fallito: {e}"));
        }
    }

    while let Some(cmd) = rx.recv().await {
        match cmd {
            CmdScor::Registra(combo, rispondi) => {
                let r = lega_scorciatoia(&app, &mut corrente, combo).await;
                let _ = rispondi.send(r);
            }
            CmdScor::Rimuovi(rispondi) => {
                if let Some(t) = corrente.take() {
                    t.abort();
                }
                let mut imp = leggi_impostazioni();
                imp.scorciatoia_screenshot = None;
                let _ = salva_impostazioni(imp);
                let _ = rispondi.send(Ok(()));
            }
        }
    }
}

#[tauri::command]
#[cfg(target_os = "linux")]
fn registra_scorciatoia(combo: String, sc: State<Scorciatoie>) -> Result<String, String> {
    let (tx, rx) = channel();
    sc.0
        .send(CmdScor::Registra(combo, tx))
        .map_err(|_| "il filo scorciatoie non risponde")?;
    rx.recv_timeout(Duration::from_secs(60))
        .map_err(|_| "nessuna risposta dal portale (timeout)".to_string())?
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
fn registra_scorciatoia(_combo: String, _sc: State<Scorciatoie>) -> Result<String, String> {
    Err("scorciatoie globali: per ora solo Linux".into())
}

#[tauri::command]
#[cfg(target_os = "linux")]
fn rimuovi_scorciatoia(sc: State<Scorciatoie>) -> Result<(), String> {
    let (tx, rx) = channel();
    sc.0
        .send(CmdScor::Rimuovi(tx))
        .map_err(|_| "il filo scorciatoie non risponde")?;
    rx.recv_timeout(Duration::from_secs(10))
        .map_err(|_| "nessuna risposta (timeout)".to_string())?
}

#[tauri::command]
#[cfg(not(target_os = "linux"))]
fn rimuovi_scorciatoia(_sc: State<Scorciatoie>) -> Result<(), String> {
    Err("scorciatoie globali: per ora solo Linux".into())
}

// ---------------------------------------------------------------- invia a DaProd

/// Sorveglia la coda di import: ogni 400 ms svuota `Libreria/.daprod/queue`,
/// copia i file in «In arrivo» e avvisa la UI con l'evento `daprod-imported`.
fn watch_queue(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(400));
        let root = library_root();
        let Ok(queued) = drain_queue(&root) else {
            continue;
        };
        if queued.is_empty() {
            continue;
        }
        match import_paths(queued, None) {
            Ok(saved) => {
                let _ = app.emit("daprod-imported", saved);
            }
            Err(err) => {
                let _ = app.emit("daprod-errore", err);
            }
        }
    });
}

// ---------------------------------------------------------------- vassoio

fn toggle_finestra(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_minimized().unwrap_or(false) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        } else if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

/// Esce dalla modalità eterea: la finestra torna interagibile e la UI si sincronizza.
fn esci_da_eterea(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_ignore_cursor_events(false);
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    let mut imp = leggi_impostazioni();
    imp.eterea = false;
    let _ = salva_impostazioni(imp);
    let _ = app.emit("eterea-cambiata", false);
}

// ---------------------------------------------------------------- app

pub fn run() {
    // WebKitGTK 2.5x su Wayland non consegna più i percorsi dei file trascinati
    // (bug webkit 320301/323277). Su X11/XWayland il drag GTK funziona: usiamo
    // x11 se l'utente non ha già scelto un backend e un display X c'è davvero.
    #[cfg(target_os = "linux")]
    if std::env::var_os("GDK_BACKEND").is_none() && std::env::var_os("DISPLAY").is_some() {
        // SAFETY: siamo single-thread prima dell'avvio di GTK.
        unsafe { std::env::set_var("GDK_BACKEND", "x11") };
    }

    // «Invia a DaProd»: se un'istanza gira già, lascia i percorsi nella coda e
    // chiuditi; se non gira nessuno, resta tu e raccogli la coda all'avvio.
    let args: Vec<String> = std::env::args().collect();
    if let Some(pos) = args.iter().position(|a| a == "--import") {
        let paths: Vec<String> = args[pos + 1..]
            .iter()
            .filter(|p| !p.starts_with('-'))
            .cloned()
            .collect();
        let root = library_root();
        let _ = ensure_library(&root);
        if !paths.is_empty() {
            let _ = enqueue_import(&root, &paths);
        }
        if running_pid(&root).is_some() {
            std::process::exit(0);
        }
    }

    let (tx, rx) = channel::<Comando>();
    std::thread::Builder::new()
        .name("audio".into())
        .spawn(|| filo_audio(rx))
        .expect("impossibile avviare il filo audio");

    let registro = RegistroAppunti(Arc::new(Mutex::new(StatoAppunti {
        lista: carica_appunti(),
        ultimo: None,
    })));
    let (scorciatoie_tx, scorciatoie_rx) = tokio::sync::mpsc::unbounded_channel::<CmdScor>();
    let scorciatoie_tx2 = scorciatoie_tx.clone();

    tauri::Builder::default()
        .manage(Audio(tx))
        .manage(registro.clone())
        .manage(Scorciatoie(scorciatoie_tx))
        .setup(move |app| {
            let root = library_root();
            let _ = write_pid(&root);
            watch_queue(app.handle().clone());

            // storia dei copia-incolla (timer GTK nel filo principale)
            #[cfg(target_os = "linux")]
            avvia_raccolta_appunti(app.handle().clone(), registro.clone());

            // scorciatoia globale per gli screenshot (portale XDG, filo tokio)
            #[cfg(target_os = "linux")]
            filo_scorciatoie(app.handle().clone(), scorciatoie_rx);
            #[cfg(not(target_os = "linux"))]
            drop(scorciatoie_rx);

            // icona nel vassoio (dove il desktop la supporta): clic = mostra/nascondi,
            // e via dalla modalità eterea se attiva
            let mostra = MenuItem::with_id(app, "mostra", "Mostra / Nascondi", true, None::<&str>)?;
            let ferma_audio =
                MenuItem::with_id(app, "ferma", "Ferma l'audio", true, None::<&str>)?;
            let esci_eterea = MenuItem::with_id(
                app,
                "eterea-off",
                "Esci da modalità eterea",
                true,
                None::<&str>,
            )?;
            let esci = MenuItem::with_id(app, "esci", "Esci", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&mostra, &ferma_audio, &esci_eterea, &esci])?;
            let tray = TrayIconBuilder::with_id("daprod")
                .icon(
                    app.default_window_icon()
                        .cloned()
                        .ok_or("icona di default mancante")?,
                )
                .menu(&menu)
                .show_menu_on_left_click(false)
                .tooltip("DaProdProduzioni")
                .on_menu_event(|app, ev| match ev.id.as_ref() {
                    "mostra" => toggle_finestra(app),
                    "ferma" => {
                        if let Some(audio) = app.try_state::<Audio>() {
                            let _ = audio.0.send(Comando::Ferma);
                        }
                        let _ = app.emit("audio-fermo-da-tray", ());
                    }
                    "eterea-off" => esci_da_eterea(app),
                    "esci" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = ev
                    {
                        let app = tray.app_handle();
                        // clic sull'icona nella barra: se eterea, la toglie
                        if leggi_impostazioni().eterea {
                            esci_da_eterea(app);
                        } else {
                            toggle_finestra(app);
                        }
                    }
                })
                .build(app)?;
            // TrayIcon è reference-counted: l'icona sparisce quando l'ultima
            // istanza viene droppata. Dimenticala di proposito: vivrà quanto il
            // processo.
            std::mem::forget(tray);

            // Via d'uscita dalla modalità eterea anche senza vassoio: se la
            // finestra riceve il focus (clic sull'icona nella barra delle
            // applicazioni, Alt+Tab…), l'eterea si spegne da sola.
            if let Some(w) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                w.on_window_event(move |ev| {
                    if let tauri::WindowEvent::Focused(true) = ev {
                        if leggi_impostazioni().eterea {
                            esci_da_eterea(&handle);
                        }
                    }
                });
            }

            // autotest: DAPROD_AUTOTEST=/p1,/p2 simula un drop dopo 5 s
            if let Ok(paths) = std::env::var("DAPROD_AUTOTEST") {
                let handle: AppHandle = app.handle().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(5));
                    let list: Vec<String> = paths
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                    let _ = handle.emit("autotest", list);
                });
            }

            // autotest scorciatoia: DAPROD_PROVA_SCOR=F9 prova il bind del portale
            #[cfg(target_os = "linux")]
            if let Ok(combo) = std::env::var("DAPROD_PROVA_SCOR") {
                let tx = scorciatoie_tx2.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_secs(6));
                    let (rtx, rrx) = channel();
                    let _ = tx.send(CmdScor::Registra(combo, rtx));
                    match rrx.recv_timeout(Duration::from_secs(25)) {
                        Ok(Ok(desc)) => println!("SCOR-TEST OK: {desc}"),
                        Ok(Err(e)) => println!("SCOR-TEST ERRORE: {e}"),
                        Err(e) => println!("SCOR-TEST TIMEOUT: {e}"),
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            info_file,
            riproduci,
            pausa,
            riprendi,
            ferma,
            volume,
            salta_a,
            stato_audio,
            leggi_testo,
            apri_esterno,
            app_version,
            traccia_debug,
            list_cassetti,
            create_cassetto,
            rename_cassetto,
            delete_file,
            elimina_se_in_libreria,
            import_paths,
            percorso_libreria,
            list_files,
            convert_file,
            rotate_image,
            trim_media,
            read_testo,
            write_testo,
            delete_cassetto,
            list_appunti,
            elimina_appunto,
            svuota_appunti,
            copia_appunto,
            leggi_impostazioni,
            salva_impostazioni,
            registra_scorciatoia,
            rimuovi_scorciatoia,
            screenshot_area,
            copia_immagine_appunti,
            salva_binario,
            esci_app,
            associa_file
        ])
        .run(tauri::generate_context!())
        .expect("errore avviando DaProd Avatar");
}
