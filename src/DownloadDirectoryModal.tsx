import { useState } from "react";
import { api } from "./bridge";
import { Modal } from "./Modal";

export function DownloadDirectoryModal({directory, onClose, onSaved}: {
  directory: string; onClose: () => void; onSaved: () => Promise<void>;
}) {
  const [path, setPath] = useState(directory);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const act = async (action: () => Promise<void>) => {
    setBusy(true); setError("");
    try { await action(); } catch (e) { setError(e instanceof Error ? e.message : String(e)); }
    finally { setBusy(false); }
  };
  return <Modal title="默认下载根目录" subtitle="新建下载会按平台和作品保存在此目录下。已有任务继续使用原来的路径。" onClose={onClose} busy={busy}>
    <form onSubmit={e => { e.preventDefault(); void act(async () => {
      const current = await api.snapshot();
      await api.saveSettings({...current.settings, downloadDir: path.trim()});
      await onSaved();
      onClose();
    }); }}>
      <label>下载根目录<input style={{width:"100%", margin:"10px 0"}} value={path} onChange={e => setPath(e.target.value)} disabled={busy} placeholder="选择文件夹或输入完整路径" /></label>
      <button type="button" className="button secondary full" disabled={busy} onClick={() => void act(async () => {
        const selected = await api.pickFolder();
        if (selected) setPath(selected);
      })}>选择文件夹</button>
      {error && <p className="inline-error" role="alert">{error}</p>}
      <button type="submit" className="button primary full" style={{marginTop:16}} disabled={busy || !path.trim()}>{busy ? "处理中…" : "保存为默认目录"}</button>
    </form>
  </Modal>;
}
