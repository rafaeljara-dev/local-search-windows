//! Local speech indexing and timestamp playback. No network inference.
use crate::{backend::Job, highlight::QueryTerms};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    os::windows::process::CommandExt,
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

pub const PROFILE: &str = "whisper-b5454-base-es-cues-v1";
pub const MAX_DURATION_MS: u64 = 30 * 60 * 1000;

#[derive(Clone, Serialize, Deserialize)]
pub struct TimedToken {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Cue {
    pub start_ms: u64,
    pub end_ms: u64,
    pub tokens: Vec<TimedToken>,
}
impl Cue {
    pub fn focus(&self, query: &str) -> u64 {
        let text: String = self.tokens.iter().map(|t| t.text.as_str()).collect();
        let Some(range) = QueryTerms::new(query).ranges(&text).into_iter().next() else {
            return self.start_ms;
        };
        let mut offset = 0;
        for token in &self.tokens {
            let end = offset + token.text.len();
            if offset < range.end && end > range.start {
                return token.start_ms.clamp(self.start_ms, self.end_ms);
            }
            offset = end;
        }
        self.start_ms
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct VideoInfo {
    pub duration_ms: u64,
    pub poster: Option<PathBuf>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Passage {
    pub text: String,
    pub cue: Cue,
}
#[derive(Serialize, Deserialize)]
pub struct PreparedVideo {
    pub stamp: String,
    pub info: VideoInfo,
    pub passages: Vec<Passage>,
}

struct TemporaryAudio(PathBuf);
impl Drop for TemporaryAudio {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub fn is_video(path: &Path) -> bool {
    path.extension().and_then(|s| s.to_str()).is_some_and(|s| {
        matches!(
            s.to_ascii_lowercase().as_str(),
            "mp4" | "mov" | "mkv" | "webm" | "avi" | "m4v"
        )
    })
}
pub fn timestamp(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{:02}:{:02}", seconds / 60, seconds % 60)
    }
}

pub struct Player {
    child: Child,
    _job: Job,
}
impl Player {
    fn spawn(command: &mut Command) -> Result<Self, String> {
        let mut child = command
            .creation_flags(0x08000000)
            .stdin(Stdio::null())
            .spawn()
            .map_err(|e| {
                format!(
                    "No se pudo iniciar {}: {e}",
                    command.get_program().to_string_lossy()
                )
            })?;
        let job = match Job::attach(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error);
            }
        };
        Ok(Self { child, _job: job })
    }
    pub fn finished(&mut self) -> Option<Result<(), String>> {
        match self.child.try_wait() {
            Ok(Some(status)) if status.success() => Some(Ok(())),
            Ok(Some(status)) => Some(Err(format!(
                "El reproductor terminó con error ({status}). Revisa app-data/video-player.log."
            ))),
            Err(error) => Some(Err(error.to_string())),
            Ok(None) => None,
        }
    }
}
impl Drop for Player {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn play(root: &Path, path: &Path, at_ms: u64) -> Result<Player, String> {
    if !is_video(path) || !path.is_file() {
        return Err("El video ya no está disponible. Sincroniza la colección.".into());
    }
    let log =
        fs::File::create(root.join("app-data/video-player.log")).map_err(|e| e.to_string())?;
    let mut command = playback_command(path, at_ms);
    command.stdout(Stdio::null()).stderr(log);
    Player::spawn(&mut command)
}
fn playback_command(path: &Path, at_ms: u64) -> Command {
    let mut command = Command::new("ffplay.exe");
    command
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-autoexit",
            "-window_title",
            "Buscador local · Video",
            "-x",
            "960",
            "-y",
            "600",
            "-ss",
        ])
        .arg(format!("{:.3}", at_ms.saturating_sub(1000) as f64 / 1000.0))
        .arg(path);
    command
}

fn run(
    command: &mut Command,
    dir: &Path,
    stage: &str,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<Vec<u8>, String> {
    if cancel.load(Ordering::Relaxed) {
        return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
    }
    let out_path = dir.join(format!("{stage}.stdout.log"));
    let err_path = dir.join(format!("{stage}.stderr.log"));
    command
        .stdout(fs::File::create(&out_path).map_err(|e| e.to_string())?)
        .stderr(fs::File::create(&err_path).map_err(|e| e.to_string())?);
    let mut running = Player::spawn(command)?;
    let start = Instant::now();
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
        }
        match running.child.try_wait().map_err(|e| e.to_string())? {
            Some(status) if status.success() => break,
            Some(status) => {
                let mut detail = String::new();
                let _ = fs::File::open(&err_path)
                    .and_then(|f| f.take(2000).read_to_string(&mut detail));
                return Err(format!("Falló {stage} ({status}): {detail}"));
            }
            None => {}
        }
        if start.elapsed() > timeout {
            return Err(format!(
                "{stage} superó el tiempo máximo; se detuvo el proceso."
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    fs::read(out_path).map_err(|e| e.to_string())
}

pub fn parse_transcript(value: &Value, duration_ms: u64) -> Result<Vec<Passage>, String> {
    let segments = value["transcription"]
        .as_array()
        .ok_or("Transcripción inválida: faltan fragmentos.")?;
    let mut passages = Vec::new();
    for segment in segments {
        let text = segment["text"].as_str().unwrap_or("").trim();
        if text.is_empty() {
            continue;
        }
        let start = segment["offsets"]["from"]
            .as_u64()
            .ok_or("Tiempo inicial inválido.")?;
        let end = segment["offsets"]["to"]
            .as_u64()
            .ok_or("Tiempo final inválido.")?
            .min(duration_ms);
        if start >= end {
            return Err("La transcripción tiene tiempos fuera de rango.".into());
        }
        let mut tokens = Vec::new();
        if let Some(raw) = segment["tokens"].as_array() {
            for token in raw {
                if token["id"].as_u64().is_none_or(|id| id >= 50257) {
                    continue;
                }
                let Some(token_text) = token["text"].as_str() else {
                    continue;
                };
                let from = token["offsets"]["from"]
                    .as_u64()
                    .unwrap_or(start)
                    .clamp(start, end);
                let to = token["offsets"]["to"]
                    .as_u64()
                    .unwrap_or(end)
                    .clamp(from, end);
                tokens.push(TimedToken {
                    text: token_text.into(),
                    start_ms: from,
                    end_ms: to,
                });
            }
        }
        // Keep passages within the embedding context. Token boundaries retain the
        // source timestamps; the original text is never replaced by generated prose.
        if text.chars().count() > 600 && !tokens.is_empty() {
            let mut group: Vec<TimedToken> = Vec::new();
            let mut length = 0;
            for token in tokens {
                if length > 450 && token.text.starts_with(char::is_whitespace) {
                    push_group(&mut passages, std::mem::take(&mut group));
                    length = 0;
                }
                length += token.text.chars().count();
                group.push(token);
            }
            push_group(&mut passages, group);
        } else {
            passages.push(Passage {
                text: text.into(),
                cue: Cue {
                    start_ms: start,
                    end_ms: end,
                    tokens,
                },
            });
        }
    }
    if passages.is_empty() {
        return Err("No se detectó habla para indexar este video.".into());
    }
    Ok(passages)
}
fn push_group(passages: &mut Vec<Passage>, tokens: Vec<TimedToken>) {
    if let (Some(first), Some(last)) = (tokens.first(), tokens.last()) {
        let text: String = tokens.iter().map(|t| t.text.as_str()).collect();
        if !text.trim().is_empty() {
            passages.push(Passage {
                text: text.trim().into(),
                cue: Cue {
                    start_ms: first.start_ms,
                    end_ms: last.end_ms.max(first.start_ms),
                    tokens,
                },
            });
        }
    }
}

pub fn prepare(
    root: &Path,
    path: &Path,
    stamp: &str,
    cancel: &AtomicBool,
    progress: &impl Fn(String),
) -> Result<PreparedVideo, String> {
    if !is_video(path) || !path.is_file() {
        return Err("Elige un video local MP4, MOV, MKV, WebM, AVI o M4V.".into());
    }
    let runtime = root.join("tools/whisper-b5454/Release/whisper-cli.exe");
    let model = root.join("models/ggml-base.bin");
    if !runtime.is_file() || !model.is_file() {
        return Err(
            "Falta el transcriptor local o su modelo de voz. Revisa la instalación.".into(),
        );
    }
    // Deterministic FNV-1a key; the cached manifest also validates the exact path/stamp.
    let key = format!("{}|{stamp}", path.to_string_lossy());
    let hash = key.bytes().fold(0xcbf29ce484222325u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x100000001b3)
    });
    let dir = root.join("app-data/videos").join(format!("{hash:016x}"));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let cached = dir.join("prepared.json");
    if let Ok(data) = fs::read(&cached) {
        if let Ok(prepared) = serde_json::from_slice::<PreparedVideo>(&data) {
            if prepared.stamp == key {
                return Ok(prepared);
            }
        }
    }
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    progress(format!("Leyendo video · {name}"));
    let metadata: Value = serde_json::from_slice(&run(
        Command::new("ffprobe.exe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration:stream=codec_type",
                "-of",
                "json",
            ])
            .arg(path),
        &dir,
        "información",
        cancel,
        Duration::from_secs(30),
    )?)
    .map_err(|e| e.to_string())?;
    let duration = metadata["format"]["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite() && *v > 0.0)
        .ok_or("No se pudo leer la duración del video.")?;
    let duration_ms = (duration * 1000.0).round() as u64;
    if duration_ms > MAX_DURATION_MS {
        return Err("Por ahora se admiten videos de hasta 30 minutos.".into());
    }
    let streams = metadata["streams"]
        .as_array()
        .ok_or("Video sin pistas válidas.")?;
    if !streams.iter().any(|s| s["codec_type"] == "audio") {
        return Err(
            "Este video no tiene audio. La búsqueda visual de fotogramas aún no está conectada."
                .into(),
        );
    }
    if !streams.iter().any(|s| s["codec_type"] == "video") {
        return Err("El archivo no contiene una pista de video.".into());
    }
    let wav = dir.join("audio.wav");
    let temporary_audio = TemporaryAudio(wav.clone());
    progress(format!("Extrayendo audio · {name}"));
    run(
        Command::new("ffmpeg.exe")
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
            .arg(path)
            .args([
                "-map",
                "0:a:0",
                "-vn",
                "-ar",
                "16000",
                "-ac",
                "1",
                "-c:a",
                "pcm_s16le",
            ])
            .arg(&wav),
        &dir,
        "audio",
        cancel,
        Duration::from_secs(180),
    )?;
    let transcript = dir.join("transcript");
    progress(format!(
        "Transcribiendo voz localmente · {name} · {}",
        timestamp(duration_ms)
    ));
    let transcribed = run(
        Command::new(runtime)
            .arg("-m")
            .arg(model)
            .arg("-f")
            .arg(&wav)
            .args(["-l", "es", "-t", "2", "-ng", "-ojf", "-of"])
            .arg(&transcript),
        &dir,
        "transcripción",
        cancel,
        Duration::from_secs(1200),
    );
    drop(temporary_audio);
    transcribed?;
    let value: Value = serde_json::from_slice(
        &fs::read(transcript.with_extension("json")).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let passages = parse_transcript(&value, duration_ms)?;
    progress(format!("Preparando miniatura · {name}"));
    let poster = dir.join("poster.jpg");
    let poster_result = run(
        Command::new("ffmpeg.exe")
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
            .arg(path)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=640:360:force_original_aspect_ratio=decrease",
                "-q:v",
                "3",
            ])
            .arg(&poster),
        &dir,
        "miniatura",
        cancel,
        Duration::from_secs(30),
    );
    if cancel.load(Ordering::Relaxed) {
        return Err("Preparación cancelada. Tu colección anterior se conserva.".into());
    }
    let prepared = PreparedVideo {
        stamp: key,
        info: VideoInfo {
            duration_ms,
            poster: poster_result.ok().map(|_| poster),
        },
        passages,
    };
    let data = serde_json::to_vec(&prepared).map_err(|e| e.to_string())?;
    fs::write(dir.join("prepared.pending.json"), data).map_err(|e| e.to_string())?;
    fs::rename(dir.join("prepared.pending.json"), cached).map_err(|e| e.to_string())?;
    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn transcript_tokens_locate_whole_unicode_words_and_fallback() {
        let value = json!({"transcription":[{"text":"Aquí en el contrato","offsets":{"from":1000,"to":9000},"tokens":[
            {"id":50364,"text":"[_BEG_]","offsets":{"from":0,"to":0}},
            {"id":1,"text":"Aquí en el ","offsets":{"from":1000,"to":4000}},
            {"id":2,"text":"contra","offsets":{"from":4200,"to":4500}},
            {"id":3,"text":"to","offsets":{"from":4500,"to":4900}}]}]});
        let passages = parse_transcript(&value, 10000).unwrap();
        assert_eq!(passages[0].cue.focus("contrato"), 4200);
        assert_eq!(passages[0].cue.focus("contratos"), 1000);
        assert_eq!(passages[0].cue.focus("aquí"), 1000);
        assert_eq!(passages[0].cue.tokens.len(), 3);
        assert!(parse_transcript(&value, 500).is_err());
        assert!(parse_transcript(&json!({"transcription":[]}), 10000).is_err());
    }
    #[test]
    fn playback_keeps_paths_as_one_argument_and_adds_context() {
        let path = Path::new("C:/videos/clip con espacios & datos.mp4");
        let command = playback_command(path, 2490);
        let args: Vec<_> = command
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args.last().unwrap(), &path.to_string_lossy());
        assert_eq!(args[args.len() - 2], "1.490");
        assert_eq!(timestamp(82430), "01:22");
        assert_eq!(timestamp(3_663_000), "1:01:03");
    }
}
