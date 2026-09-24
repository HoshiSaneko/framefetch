import { PlatformIcon } from "./PlatformIcon";
import type { PlatformId } from "./platforms";

export function PlatformCardHeader({ id, name, connected, available = true }: {
  id: PlatformId;
  name: string;
  connected: boolean;
  available?: boolean;
}) {
  return <div className="platform-card-top">
    <div className="platform-card-identity">
      <div className={`platform-avatar ${id}`}><PlatformIcon id={id} size={28}/></div>
      <h2>{name}</h2>
    </div>
    <span className="platform-connection-state" data-connected={connected && available}>
      <i aria-hidden="true"/>{available ? connected ? "已连接" : "可连接" : "尚未接入"}
    </span>
  </div>;
}
