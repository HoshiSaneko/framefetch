import { Notice } from "./Notice";

export function PlatformNotice({platform, message, onClose, onRetry}: {
  platform: string;
  message: string;
  onClose: () => void;
  onRetry?: () => void;
}) {
  return <Notice kind="error" title={`${platform} · 连接提示`} message={message}
    onClose={onClose} closeLabel={`关闭${platform}提示`}
    action={onRetry ? {label: "重新登录", onClick: onRetry} : undefined}/>;
}
