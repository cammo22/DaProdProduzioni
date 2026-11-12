use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DEFAULT_DRAWER: &str = "In arrivo";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Kind {
    Image,
    Video,
    Audio,
    Text,
    Other,
}

pub fn kind_of(path: &Path) -> Kind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "tif" | "tiff" => Kind::Image,
        "mp4" | "mkv" | "webm" | "mov" | "avi" | "m4v" => Kind::Video,
        "mp3" | "wav" | "flac" | "ogg" | "m4a" | "aac" | "opus" => Kind::Audio,
        "txt" | "md" | "csv" | "json" | "srt" | "log" | "html" | "css" | "rs" | "toml" | "xml" => {
            Kind::Text
        }
        _ => Kind::Other,
    }
}

pub fn library_root() -> PathBuf {
    if let Ok(p) = std::env::var("DAPROD_LIBRARY") {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "windows")]
    {
        let base = std::env::var("USERPROFILE").unwrap_or_else(|_| "C:\\Users\\cammo".into());
        return PathBuf::from(base).join("Documents").join("DaProd").join("Libreria");
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/cammo".into());
        PathBuf::from(home).join("Scrivania").join("DaProd").join("Libreria")
    }
}

pub fn ensure_library(root: &Path) -> io::Result<()> {
    fs::create_dir_all(root.join(DEFAULT_DRAWER))?;
    fs::create_dir_all(root.join(".daprod").join("queue"))?;
    fs::create_dir_all(root.join(".daprod").join("thumbs"))?;
    Ok(())
}

pub fn sanitize_drawer(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() || name == "." || name == ".." || name.starts_with('.') {
        return Err("nome cassetto non valido".into());
    }
    if name.contains('/') || name.contains('\\') || name.contains('\0') {
        return Err("il nome non può contenere slash".into());
    }
    if name.chars().count() > 80 {
        return Err("nome troppo lungo".into());
    }
    Ok(name.to_string())
}

pub fn create_unnamed_drawer(root: &Path) -> Result<String, String> {
    ensure_library(root).map_err(|e| e.to_string())?;
    let mut n = 1u32;
    let id = loop {
        let candidate = format!("n-{n}");
        if !root.join(&candidate).exists() {
            break candidate;
        }
        n += 1;
        if n > 100_000 {
            return Err("troppi cassetti".into());
        }
    };
    fs::create_dir_all(root.join(&id)).map_err(|e| e.to_string())?;
    set_label(root, &id, "")?;
    Ok(id)
}

pub fn rename_drawer(root: &Path, id: &str, label: &str) -> Result<(), String> {
    let id = sanitize_drawer(id)?;
    if !root.join(&id).is_dir() {
        return Err("cassetto inesistente".into());
    }
    let label = label.trim();
    if !label.is_empty() {
        sanitize_drawer(label)?;
    }
    set_label(root, &id, label)
}

pub fn drawer_label(root: &Path, id: &str) -> String {
    let labels = load_labels(root);
    if let Some(label) = labels.get(id) {
        return label.clone();
    }
    if id.starts_with("n-") {
        String::new()
    } else {
        id.to_string()
    }
}

fn labels_path(root: &Path) -> PathBuf {
    root.join(".daprod").join("labels.json")
}

fn load_labels(root: &Path) -> std::collections::HashMap<String, String> {
    let Ok(text) = fs::read_to_string(labels_path(root)) else {
        return std::collections::HashMap::new();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

fn set_label(root: &Path, id: &str, label: &str) -> Result<(), String> {
    let _ = fs::create_dir_all(root.join(".daprod"));
    let mut labels = load_labels(root);
    labels.insert(id.to_string(), label.to_string());
    let text = serde_json::to_string_pretty(&labels).map_err(|e| e.to_string())?;
    fs::write(labels_path(root), text).map_err(|e| e.to_string())
}

pub fn delete_inside(root: &Path, path: &Path) -> Result<(), String> {
    if !inside_library(root, path) {
        return Err("il file non è in un cassetto".into());
    }
    let root_canon = root.canonicalize().map_err(|e| e.to_string())?;
    let path_canon = path.canonicalize().map_err(|e| e.to_string())?;
    if path_canon == root_canon || path_canon.starts_with(root_canon.join(".daprod")) {
        return Err("questo non si elimina".into());
    }
    if path.is_dir() {
        fs::remove_dir_all(path).map_err(|e| e.to_string())?;
    } else {
        fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn create_drawer(root: &Path, name: &str) -> Result<PathBuf, String> {
    let name = sanitize_drawer(name)?;
    let path = root.join(&name);
    fs::create_dir_all(&path).map_err(|e| e.to_string())?;
    Ok(path)
}

pub fn list_drawers(root: &Path) -> io::Result<Vec<String>> {
    ensure_library(root)?;
    let mut names = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        names.push(name);
    }
    names.sort();
    if !names.iter().any(|n| n == DEFAULT_DRAWER) {
        names.insert(0, DEFAULT_DRAWER.to_string());
    }
    Ok(names)
}

pub fn unique_in_dir(dir: &Path, file_name: &str) -> PathBuf {
    let candidate = dir.join(file_name);
    if !candidate.exists() {
        return candidate;
    }
    let path = Path::new(file_name);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("file");
    let ext = path.extension().and_then(|s| s.to_str());
    for n in 2..10_000 {
        let name = match ext {
            Some(ext) => format!("{stem}-{n}.{ext}"),
            None => format!("{stem}-{n}"),
        };
        let candidate = dir.join(name);
        if !candidate.exists() {
            return candidate;
        }
    }
    dir.join(format!("{stem}-copy"))
}

pub fn import_path(root: &Path, drawer: &str, src: &Path) -> Result<PathBuf, String> {
    let drawer = sanitize_drawer(drawer)?;
    if !src.exists() {
        return Err(format!("non trovo {}", src.display()));
    }
    let dest_dir = root.join(&drawer);
    fs::create_dir_all(&dest_dir).map_err(|e| e.to_string())?;
    let name = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    let dest = unique_in_dir(&dest_dir, &name);
    if src.is_dir() {
        copy_dir(src, &dest).map_err(|e| e.to_string())?;
    } else {
        fs::copy(src, &dest).map_err(|e| e.to_string())?;
    }
    Ok(dest)
}

fn copy_dir(src: &Path, dest: &Path) -> io::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let to = dest.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to)?;
        }
    }
    Ok(())
}

pub fn inside_library(root: &Path, path: &Path) -> bool {
    let Ok(root) = root.canonicalize() else {
        return false;
    };
    let Ok(path) = path.canonicalize() else {
        return false;
    };
    path.starts_with(root)
}

pub fn suggestions_for(path: &Path) -> Vec<&'static str> {
    match kind_of(path) {
        Kind::Image => vec!["Converti in PNG", "Ruota di 90°", "Apri editor"],
        Kind::Video => vec!["Converti in MP4", "Estrai audio", "Taglia", "Apri editor"],
        Kind::Audio => vec!["Converti in MP3", "Converti in WAV"],
        Kind::Text => vec!["Modifica testo", "Apri editor"],
        Kind::Other => vec!["Apri editor"],
    }
}

pub fn read_text(path: &Path) -> Result<String, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes.len() > 1_000_000 {
        return Err("file di testo troppo grande per l'editor".into());
    }
    String::from_utf8(bytes).map_err(|_| "non è testo UTF-8".to_string())
}

pub fn write_text(path: &Path, text: &str) -> Result<(), String> {
    if text.len() > 1_000_000 {
        return Err("testo troppo grande".into());
    }
    fs::write(path, text).map_err(|e| e.to_string())
}

pub fn ffmpeg() -> Option<PathBuf> {
    for candidate in ["/usr/bin/ffmpeg", "/usr/sbin/ffmpeg"] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

pub fn run_ffmpeg(args: &[&str]) -> Result<(), String> {
    let bin = ffmpeg().ok_or("ffmpeg non è installato")?;
    let out = Command::new(bin)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        let tail: String = err.chars().rev().take(400).collect::<String>().chars().rev().collect();
        Err(format!("ffmpeg ha fallito: {tail}"))
    }
}

pub fn thumb_path(root: &Path, source: &Path) -> PathBuf {
    let stamp = source
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let key = format!("{:x}-{}", hash_str(&source.to_string_lossy()), stamp);
    root.join(".daprod").join("thumbs").join(format!("{key}.jpg"))
}

fn hash_str(value: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in value.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn ensure_thumb(root: &Path, source: &Path) -> Option<PathBuf> {
    match kind_of(source) {
        Kind::Image | Kind::Video => {}
        _ => return None,
    }
    let dest = thumb_path(root, source);
    if dest.is_file() {
        return Some(dest);
    }
    if let Some(parent) = dest.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let input = source.to_string_lossy().to_string();
    let output = dest.to_string_lossy().to_string();
    let args: Vec<&str> = if kind_of(source) == Kind::Video {
        vec![
            "-y", "-ss", "0.5", "-i", &input, "-frames:v", "1", "-vf", "scale=360:-1", &output,
        ]
    } else {
        vec!["-y", "-i", &input, "-vf", "scale=360:-1", &output]
    };
    // The args borrow locals; rebuild owned for the call.
    let owned: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
    let refs: Vec<&str> = owned.iter().map(String::as_str).collect();
    run_ffmpeg(&refs).ok()?;
    dest.is_file().then_some(dest)
}

pub fn thumb_data_url(root: &Path, source: &Path) -> Option<String> {
    if let Some(path) = ensure_thumb(root, source) {
        let bytes = fs::read(path).ok()?;
        return Some(format!("data:image/jpeg;base64,{}", b64(&bytes)));
    }
    if kind_of(source) == Kind::Image {
        let bytes = fs::read(source).ok()?;
        if bytes.len() > 6_000_000 {
            return None;
        }
        let mime = match source
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str()
        {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => return None,
        };
        return Some(format!("data:{mime};base64,{}", b64(&bytes)));
    }
    None
}

fn b64(bytes: &[u8]) -> String {
    const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut i = 0;
    while i + 3 <= bytes.len() {
        let n = ((bytes[i] as u32) << 16) | ((bytes[i + 1] as u32) << 8) | bytes[i + 2] as u32;
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        out.push(TABLE[((n >> 6) & 63) as usize] as char);
        out.push(TABLE[(n & 63) as usize] as char);
        i += 3;
    }
    if i < bytes.len() {
        let n = (bytes[i] as u32) << 16
            | if i + 1 < bytes.len() {
                (bytes[i + 1] as u32) << 8
            } else {
                0
            };
        out.push(TABLE[((n >> 18) & 63) as usize] as char);
        out.push(TABLE[((n >> 12) & 63) as usize] as char);
        if i + 1 < bytes.len() {
            out.push(TABLE[((n >> 6) & 63) as usize] as char);
            out.push('=');
        } else {
            out.push('=');
            out.push('=');
        }
    }
    out
}
pub fn converted_name(source: &Path, format: &str) -> String {
    let stem = source
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("file");
    format!("{stem}.{format}")
}

pub fn enqueue_import(root: &Path, paths: &[String]) -> io::Result<()> {
    ensure_library(root)?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = root.join(".daprod").join("queue").join(format!("{nanos}.txt"));
    let mut file = fs::File::create(path)?;
    for item in paths {
        writeln!(file, "{item}")?;
    }
    Ok(())
}

pub fn drain_queue(root: &Path) -> io::Result<Vec<String>> {
    let dir = root.join(".daprod").join("queue");
    fs::create_dir_all(&dir)?;
    let mut paths = Vec::new();
    let mut names: Vec<_> = fs::read_dir(&dir)?.filter_map(|e| e.ok()).collect();
    names.sort_by_key(|e| e.file_name());
    for entry in names {
        let text = fs::read_to_string(entry.path()).unwrap_or_default();
        for line in text.lines() {
            let line = line.trim();
            if !line.is_empty() {
                paths.push(line.to_string());
            }
        }
        let _ = fs::remove_file(entry.path());
    }
    Ok(paths)
}

pub fn pid_path(root: &Path) -> PathBuf {
    root.join(".daprod").join("pid")
}

pub fn write_pid(root: &Path) -> io::Result<()> {
    ensure_library(root)?;
    fs::write(pid_path(root), std::process::id().to_string())
}

pub fn running_pid(root: &Path) -> Option<u32> {
    let text = fs::read_to_string(pid_path(root)).ok()?;
    let pid: u32 = text.trim().parse().ok()?;
    if pid == std::process::id() {
        return None;
    }
    if Path::new(&format!("/proc/{pid}")).is_dir() {
        Some(pid)
    } else {
        None
    }
}

pub fn ffprobe() -> Option<PathBuf> {
    for candidate in ["/usr/bin/ffprobe", "/usr/sbin/ffprobe"] {
        let path = PathBuf::from(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

/// Durata in secondi letta da ffprobe. None se non si riesce.
pub fn ffprobe_duration(path: &Path) -> Option<f64> {
    let bin = ffprobe()?;
    let out = Command::new(bin)
        .args([
            "-v",
            "error",
            "-show_entries",
            "format=duration",
            "-of",
            "csv=p=0",
            &path.to_string_lossy(),
        ])
        .output()
        .ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    s.trim().parse::<f64>().ok().filter(|d| *d > 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("daprod-test-{n}"));
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn copy_keeps_original_and_survives_source_delete() {
        let root = temp_root();
        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir).unwrap();
        let src = src_dir.join("nota.txt");
        fs::write(&src, "ciao").unwrap();
        let dest = import_path(&root, DEFAULT_DRAWER, &src).unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "ciao");
        fs::remove_file(&src).unwrap();
        assert_eq!(fs::read_to_string(&dest).unwrap(), "ciao");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn second_copy_gets_a_new_name() {
        let root = temp_root();
        let src = root.join("foto.png");
        fs::write(&src, "a").unwrap();
        let first = import_path(&root, "Scatti", &src).unwrap();
        let second = import_path(&root, "Scatti", &src).unwrap();
        assert_ne!(first, second);
        assert!(second.ends_with("foto-2.png"));
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn unnamed_drawer_can_be_named_later() {
        let root = temp_root();
        let id = create_unnamed_drawer(&root).unwrap();
        assert!(drawer_label(&root, &id).is_empty());
        rename_drawer(&root, &id, "Vacanze").unwrap();
        assert_eq!(drawer_label(&root, &id), "Vacanze");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn rejects_bad_drawer_names() {
        assert!(sanitize_drawer("..").is_err());
        assert!(sanitize_drawer("a/b").is_err());
        assert!(sanitize_drawer(".nascosto").is_err());
        assert_eq!(sanitize_drawer("  Vacanze  ").unwrap(), "Vacanze");
    }
}
