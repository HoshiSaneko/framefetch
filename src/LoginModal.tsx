import { useEffect, useRef, useState, type FormEvent } from "react";
import {
  ArrowRight,
  ChevronRight,
  LoaderCircle,
  QrCode,
  RefreshCw,
  ShieldCheck,
  Smartphone,
} from "lucide-react";
import QRCode from "qrcode";
import { api, desktop } from "./bridge";
import { Modal } from "./Modal";
import type { Account, QrLoginResult, Settings } from "./types";

export const validApiConfig = (settings: Settings) =>
  /^\d+$/.test(settings.apiId) &&
  Number(settings.apiId) > 0 &&
  Number(settings.apiId) <= 2147483647 &&
  /^[a-f0-9]{32}$/i.test(settings.apiHash);
const message = (error: unknown) =>
  error instanceof Error ? error.message : String(error);

export function LoginModal({
  settings,
  onClose,
  onConnected,
}: {
  settings: Settings;
  onClose: () => void;
  onConnected: (account: Account) => void;
}) {
  const configured = validApiConfig(settings);
  const [config, setConfig] = useState(settings);
  const [step, setStep] = useState<"config" | "qr" | "password">(
    configured ? "qr" : "config",
  );
  const [run, setRun] = useState<number | null>(configured ? 0 : null);
  const [qr, setQr] = useState<QrLoginResult | null>(null);
  const [image, setImage] = useState("");
  const [clock, setClock] = useState(Date.now());
  const [error, setError] = useState("");
  const [password, setPassword] = useState("");
  const [hint, setHint] = useState("");
  const [busy, setBusy] = useState(false);
  const attempt = useRef<string | null>(null);
  const callbacks = useRef({ onClose, onConnected });
  callbacks.current = { onClose, onConnected };
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);

  useEffect(() => {
    if (run === null) return;
    let canceled = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const id = crypto.randomUUID();
    attempt.current = id;
    setQr(null);
    setImage("");
    setError("");
    const receive = (next: QrLoginResult) => {
      if (canceled) return;
      if (next.step === "connected" && next.account) {
        callbacks.current.onConnected(next.account);
        callbacks.current.onClose();
      } else if (next.step === "password") {
        setQr(null);
        setImage("");
        setHint(next.hint);
        setStep("password");
      } else if (next.step === "qr") {
        setQr(next);
        timer = setTimeout(poll, 1000);
      }
    };
    const poll = async () => {
      try {
        receive(await api.pollQr(id));
      } catch (e) {
        if (!canceled) {
          setError(message(e));
          setQr(null);
          setImage("");
        }
      }
    };
    api
      .startQr(config, id)
      .then(receive)
      .catch((e) => {
        if (!canceled) setError(message(e));
      });
    return () => {
      canceled = true;
      clearTimeout(timer);
      if (attempt.current === id) attempt.current = null;
      // Also cancel if the initial request finishes after the dialog was closed.
      api.cancelQr(id).catch(() => {});
    };
    // A run captures one immutable configuration; account refreshes must not rotate its code.
  }, [run]);

  useEffect(() => {
    let canceled = false;
    setImage("");
    if (qr?.url) {
      QRCode.toDataURL(qr.url, {
        width: 600,
        margin: 4,
        errorCorrectionLevel: "M",
        color: { dark: "#142c20", light: "#ffffff" },
      })
        .then((src) => {
          if (!canceled) setImage(src);
        })
        .catch(() => {
          if (!canceled) setError("二维码生成失败，请重新生成。");
        });
    }
    return () => {
      canceled = true;
    };
  }, [qr?.url]);
  useEffect(() => {
    if (step !== "qr") return;
    const timer = setInterval(() => setClock(Date.now()), 1000);
    return () => clearInterval(timer);
  }, [step]);

  const seconds = qr ? Math.max(0, qr.expiresAt - Math.floor(clock / 1000)) : 0;
  const restart = () => {
    setError("");
    setStep("qr");
    setPassword("");
    setRun((n) => (n ?? -1) + 1);
  };
  const editConfig = () => {
    setRun(null);
    setStep("config");
    setQr(null);
    setImage("");
    setError("");
    setPassword("");
  };
  const submit = async (event: FormEvent) => {
    event.preventDefault();
    setError("");
    if (step === "config") {
      if (!validApiConfig(config)) {
        setError("请输入有效的 API ID 和 32 位 API Hash。");
        return;
      }
      restart();
      return;
    }
    if (step !== "password" || !attempt.current) return;
    const id = attempt.current;
    setBusy(true);
    try {
      const response = await api.password(password, id);
      if (!alive.current || attempt.current !== id) return;
      if (response.step === "connected" && response.account) {
        callbacks.current.onConnected(response.account);
        callbacks.current.onClose();
      }
    } catch (e) {
      if (alive.current && attempt.current === id) setError(message(e));
    } finally {
      if (alive.current) {
        setBusy(false);
        setPassword("");
      }
    }
  };

  return (
    <Modal
      icon={step === "qr" ? <QrCode size={24} /> : step === "config" ? <Smartphone size={24} /> : <ShieldCheck size={24} />}
      title={
        step === "config"
          ? "设置 Telegram 连接"
          : step === "qr"
            ? "扫码登录 Telegram"
            : "输入两步验证密码"
      }
      subtitle={
        step === "config"
          ? "首次填写应用配置，之后打开即可扫码。"
          : step === "qr"
            ? "用手机 Telegram 扫描二维码，并在手机上确认。"
            : hint
              ? `密码提示：${hint}`
              : "扫码已确认，你的账号还需要两步验证密码。"
      }
      onClose={onClose}
      busy={busy}
    >
      {step === "qr" ? (
        <div className="qr-login">
          <div className="qr-frame" aria-busy={!image && !error}>
            {image && seconds > 0 && !error ? (
              <img
                src={image}
                alt="Telegram 登录二维码，请使用手机 Telegram 扫描"
              />
            ) : (
              <div className="qr-placeholder">
                {error ? (
                  <QrCode size={48} strokeWidth={1.3} />
                ) : (
                  <LoaderCircle className="spin" size={30} />
                )}
                <span>
                  {error
                    ? "二维码暂不可用"
                    : qr
                      ? "正在刷新二维码…"
                      : "正在生成二维码…"}
                </span>
                {error && (
                  <p className="qr-error" role="alert">
                    {error}
                  </p>
                )}
              </div>
            )}
          </div>
          <div className="qr-live" role="status">
            {error ? (
              "连接未完成"
            ) : image && seconds > 0 ? (
              <>
                <i />
                等待手机确认<span>{seconds} 秒后自动刷新</span>
              </>
            ) : (
              "正在连接 Telegram"
            )}
          </div>
          <ol className="qr-instructions">
            <li>
              <span>1</span>打开手机上的 Telegram
            </li>
            <li>
              <span>2</span>进入「设置」→「设备」
            </li>
            <li>
              <span>3</span>点击「扫描二维码」，扫码并确认
            </li>
          </ol>

          <div className="qr-actions">
            {error && desktop && (
              <button className="button primary" onClick={restart}>
                <RefreshCw size={15} />
                重新生成二维码
              </button>
            )}
            <button className="button secondary" onClick={editConfig}>
              连接设置
            </button>
          </div>
        </div>
      ) : (
        <form onSubmit={submit}>
          {step === "config" ? (
            <>
              <div className="qr-config-note">
                <Smartphone size={19} />
                <span>扫码完成登录，无需填写手机号或验证码。</span>
              </div>
              <div className="form-columns">
                <label>
                  API ID
                  <input
                    value={config.apiId}
                    onChange={(e) =>
                      setConfig({ ...config, apiId: e.target.value.trim() })
                    }
                    inputMode="numeric"
                    placeholder="12345678"
                    required
                  />
                </label>
                <label>
                  API Hash
                  <input
                    type="password"
                    value={config.apiHash}
                    onChange={(e) =>
                      setConfig({ ...config, apiHash: e.target.value.trim() })
                    }
                    placeholder="你的 API Hash"
                    required
                  />
                </label>
              </div>
              <p className="field-note">
                在 my.telegram.org 的 API development tools
                中创建应用，获取以上信息。
              </p>
              <details className="proxy-details">
                <summary>
                  网络代理（可选）
                  <ChevronRight size={14} />
                </summary>
                <label>
                  SOCKS5 代理
                  <input
                    value={config.proxyUrl}
                    onChange={(e) =>
                      setConfig({ ...config, proxyUrl: e.target.value })
                    }
                    placeholder="socks5://127.0.0.1:1080"
                  />
                </label>
                <p className="field-note">
                  无法直连 Telegram 时，填写本地 SOCKS5 地址。
                </p>
              </details>
            </>
          ) : (
            <label>
              两步验证密码
              <input
                type="password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                autoComplete="current-password"
                placeholder="输入密码"
                required
                autoFocus
              />
            </label>
          )}
          {error && (
            <div className="inline-error" role="alert">
              {error}
            </div>
          )}
          <button className="button primary full" disabled={busy} type="submit">
            {busy ? (
              <LoaderCircle className="spin" size={17} />
            ) : step === "config" ? (
              <QrCode size={18} />
            ) : (
              <ArrowRight size={17} />
            )}
            {busy
              ? "正在验证…"
              : step === "config"
                ? "生成登录二维码"
                : "验证并连接"}
          </button>
          {step === "password" && (
            <button
              className="qr-back"
              type="button"
              disabled={busy}
              onClick={restart}
            >
              重新扫码
            </button>
          )}
        </form>
      )}
      <p className="privacy-note">
        <ShieldCheck size={14} />
        二维码在本地生成，会话仅保存在此设备。
      </p>
    </Modal>
  );
}

