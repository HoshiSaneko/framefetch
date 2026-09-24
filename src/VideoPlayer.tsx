import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Maximize, Pause, Play, Volume2, VolumeX, ArrowUpRight } from "lucide-react";
import { Modal } from "./Modal";
import { api } from "./bridge";
import type { DownloadTask } from "./types";

const AUDIO_SETTINGS_KEY = "framefetch.previewAudio";
type PreviewAudio = {volume: number; muted: boolean};
function readPreviewAudio(): PreviewAudio {
  try {
    const saved = JSON.parse(localStorage.getItem(AUDIO_SETTINGS_KEY) || "null");
    return {
      volume: typeof saved?.volume === "number" && Number.isFinite(saved.volume) ? Math.min(1, Math.max(0, saved.volume)) : 1,
      muted: saved?.muted === true,
    };
  } catch { return {volume: 1, muted: false}; }
}

export const isImage = (name: string) => /\.(jpe?g|png|webp|gif|bmp|avif)$/i.test(name);
const time = (seconds: number) => `${Math.floor(seconds / 60)}:${Math.floor(seconds % 60).toString().padStart(2, "0")}`;
export function SeekBar({src, duration, current, disabled, onSeek}: {
  src: string; duration: number; current: number; disabled: boolean; onSeek: (value: number) => void;
}) {
  const preview = useRef<HTMLVideoElement>(null);
  const [hover, setHover] = useState<{seconds: number; left: number} | null>(null);
  const [ready, setReady] = useState(false);
  const [failed, setFailed] = useState(false);
  const seekPreview = () => {
    const el = preview.current;
    if (el && hover && el.readyState >= 1) el.currentTime = Math.min(hover.seconds, Math.max(0, duration - 0.05));
  };
  useEffect(() => {
    setReady(false);
    const timer = window.setTimeout(seekPreview, 100);
    return () => window.clearTimeout(timer);
  }, [hover, duration]);
  return <div className="video-seek-wrap" onMouseLeave={() => {setHover(null); setFailed(false);}}>
    <input className="video-seek" aria-label="播放进度" type="range" min={0} max={duration || 1} step={0.1} value={Math.min(current, duration || 1)} disabled={disabled}
      onMouseMove={e => {
        if (disabled || !duration) return;
        const rect = e.currentTarget.getBoundingClientRect();
        const x = Math.max(0, Math.min(rect.width, e.clientX - rect.left));
        setHover({seconds: x / Math.max(1, rect.width) * duration, left: Math.max(88, Math.min(rect.width - 88, x))});
      }}
      onChange={e => onSeek(Number(e.target.value))} />
    {hover && <div className="video-hover-preview" style={{left: hover.left}} role="tooltip" aria-label={`预览 ${time(hover.seconds)}`}>
      <div className="video-hover-frame">
        <video ref={preview} src={src} muted playsInline preload="metadata" aria-hidden="true" style={{opacity: ready ? 1 : 0}}
          onLoadedMetadata={seekPreview} onSeeked={() => {if (preview.current && Math.abs(preview.current.currentTime - Math.min(hover.seconds, duration - 0.05)) < 0.2) setReady(true);}}
          onLoadedData={() => {if (preview.current && Math.abs(preview.current.currentTime - Math.min(hover.seconds, duration - 0.05)) < 0.2) setReady(true);}}
          onError={() => setFailed(true)} />
        {!ready && <span>{failed ? "预览不可用" : "加载预览…"}</span>}
      </div>
      <span className="video-hover-time">{time(hover.seconds)}</span>
    </div>}
  </div>;
}
export function VideoPlayer({task: initialTask, galleryTasks, onClose}: {task: DownloadTask; galleryTasks?: DownloadTask[]; onClose: () => void}) {
  const [imageIndex, setImageIndex] = useState(0);
  const images = galleryTasks?.filter(t => isImage(t.fileName) && t.status === "completed") || [];
  const task = images.length > 1 ? images[Math.min(imageIndex, images.length - 1)] : initialTask;
  const image = isImage(task.fileName);
  const video = useRef<HTMLVideoElement>(null);
  const surface = useRef<HTMLDivElement>(null);
  const [src, setSrc] = useState("");
  const [error, setError] = useState("");
  const [playing, setPlaying] = useState(false);
  const [duration, setDuration] = useState(0);
  const [current, setCurrent] = useState(0);
  const [{volume, muted}, setAudio] = useState(readPreviewAudio);
  const updateAudio = (audio: PreviewAudio) => {
    setAudio(audio);
    try { localStorage.setItem(AUDIO_SETTINGS_KEY, JSON.stringify(audio)); } catch { /* Playback still works if storage is unavailable. */ }
  };
  // Apply saved levels before the first source can autoplay, and on source changes.
  useLayoutEffect(() => {
    if (video.current) { video.current.volume = volume; video.current.muted = muted; }
  }, [volume, muted, src, image]);
  useEffect(() => {
    let alive = true;
    setSrc(""); setError("");
    api.playbackPath(task.id).then(path => {if (alive) setSrc(convertFileSrc(path));})
      .catch(e => {if (alive) setError(typeof e === "string" ? e : "无法打开视频");});
    const element = video.current;
    return () => { alive = false; if (element) {element.pause(); element.removeAttribute("src"); element.load();} };
  }, [task.id]);
  const toggle = () => {
    const el = video.current;
    if (!el || !src) return;
    if (el.paused) void el.play().catch(() => setError("当前视频无法播放，请尝试系统播放器。"));
    else el.pause();
  };
  return <Modal title={task.title || task.fileName} className="video-player-modal" onClose={onClose}>
    <div ref={surface} className="video-player-surface">
      <div className="video-stage">
        {image ? (src && <img className="media-preview-image" src={src} alt={task.title || task.fileName} onError={() => setError("图片加载失败，请检查文件是否存在。")}/>) : <video ref={video} src={src || undefined} poster={task.thumbnail || undefined} autoPlay playsInline preload="metadata"
          onClick={toggle} onPlay={() => setPlaying(true)} onPause={() => setPlaying(false)} onEnded={() => setPlaying(false)}
          onLoadedMetadata={() => setDuration(Number.isFinite(video.current!.duration) ? video.current!.duration : 0)}
          onTimeUpdate={() => setCurrent(video.current!.currentTime)}
          onError={() => setError("当前视频无法播放，请尝试系统播放器。")} />}
        {!src && !error && <span className="video-loading">{image ? "正在打开图片…" : "正在打开视频…"}</span>}
        {error && <div className="video-error" role="alert"><p>{error}</p><button className="button secondary" onClick={() => void api.openFile(task.id).catch(() => setError("无法打开文件，请检查文件是否存在。"))}><ArrowUpRight size={17} />{image ? "系统应用打开" : "系统播放器打开"}</button></div>}
      </div>
      {!image && <div className="video-controls">
        <SeekBar src={src} duration={duration} current={current} disabled={!duration || !!error} onSeek={value => {if (video.current) video.current.currentTime = value; setCurrent(value);}} />
        <div className="video-control-row">
          <button className="video-play" aria-label={playing ? "暂停播放" : "播放视频"} disabled={!src || !!error} onClick={toggle}>{playing ? <Pause size={23} /> : <Play size={23} />}</button>
          <span className="video-time">{time(current)} <span>/ {time(duration)}</span></span>
          <div className="video-volume"><button aria-label={muted ? "取消静音" : "静音"} onClick={() => updateAudio({volume, muted: !muted})}>{muted || volume === 0 ? <VolumeX size={20} /> : <Volume2 size={20} />}</button>
            <input aria-label="音量" type="range" min={0} max={1} step={0.01} value={muted ? 0 : volume} onChange={e => updateAudio({volume: Number(e.target.value), muted: false})} />
          </div>
          <button aria-label="全屏播放" onClick={() => {const operation = document.fullscreenElement ? document.exitFullscreen() : surface.current?.requestFullscreen(); void operation?.catch(() => setError("当前窗口不支持全屏播放。"));}}><Maximize size={20} /></button>
        </div>
      </div>}
    </div>
    {images.length > 1 && <div className="modal-actions gallery-controls"><button className="button secondary" disabled={imageIndex === 0} onClick={() => setImageIndex(i=>i-1)}>上一张</button><span>{imageIndex + 1} / {images.length}</span><button className="button secondary" disabled={imageIndex >= images.length - 1} onClick={() => setImageIndex(i=>i+1)}>下一张</button></div>}
  </Modal>;
}
