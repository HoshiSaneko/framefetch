import { useState, type MouseEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { desktop } from "./bridge";
import { Info, ShieldCheck } from "lucide-react";
import appConfig from "../src-tauri/tauri.conf.json";
import { Modal } from "./Modal";

export function AboutModal({ onClose }: { onClose: () => void }) {
  const [linkError, setLinkError] = useState("");
  const openLink = (event: MouseEvent<HTMLAnchorElement>) => {
    if (!desktop) return;
    event.preventDefault();
    setLinkError("");
    invoke("open_external_link", { url: event.currentTarget.href }).catch(() => {
      setLinkError("无法打开浏览器，请复制链接后访问。");
    });
  };
  return <Modal title="说明" icon={<Info size={24} />} onClose={onClose} className="about-modal">
    <div className="about-brand"><strong>拾帧 <span>FrameFetch</span></strong><span className="about-version">v{appConfig.version}</span></div>
    <p className="about-purpose">一款桌面媒体下载与整理工具。支持 Telegram、抖音、哔哩哔哩和小红书，将选定的视频、图片、封面及音频保存到本地，按平台和作品归档。</p>
    <section className="about-privacy" aria-labelledby="about-privacy-title">
      <h3 id="about-privacy-title"><ShieldCheck size={19} />账号与隐私</h3>
      <p>登录仅用于验证平台访问权限、显示账号信息，以及读取和下载你选择的内容（包括所选收藏、合集等）。账号不会用于与这些功能无关的用途。</p>
      <p>登录会话保存在本机，访问平台时用于身份验证。本工具不会主动发布内容、发送消息、点赞或关注，也不会将登录凭据上传给项目作者。</p>
      <p>你可以随时在「平台连接」中断开账号连接。</p>
    </section>
    <dl className="about-details">
      <div><dt>当前版本</dt><dd>{appConfig.version}</dd></div>
      <div><dt>作者</dt><dd><a href="https://github.com/HoshiSaneko" target="_blank" rel="noopener noreferrer" onClick={openLink}>Saneko</a></dd></div>
      <div className="about-detail-wide"><dt>项目地址</dt><dd><a href="https://github.com/HoshiSaneko/framefetch" target="_blank" rel="noopener noreferrer" onClick={openLink}>github.com/HoshiSaneko/framefetch</a></dd></div>
      <div className="about-detail-wide"><dt>说明更新</dt><dd><time dateTime="2026-09-24">2026-09-24</time></dd></div>
    </dl>
    {linkError && <p role="alert">{linkError}</p>}
    <div className="modal-actions"><button type="button" className="button primary" onClick={onClose}>知道了</button></div>
  </Modal>;
}
