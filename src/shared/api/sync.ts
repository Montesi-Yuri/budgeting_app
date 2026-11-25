import { invoke } from "@tauri-apps/api/core";

/** Per-Device Sync settings and whether the listener runs. */
export type SyncStatus = {
  enabled: boolean;
  port: number;
  listening: boolean;
  addrs: string[];
  /** This Device announces itself on the local network (mDNS), so the phone finds it after an address change. */
  announced: boolean;
  /** The Windows Firewall rule for Sync (private networks only). */
  firewall: "allowed" | "missing" | "not_needed";
  /** The firewall explanation was answered already, so it isn't shown again. */
  firewall_asked: boolean;
};

/** "Pair new device": the text to paste on the other Device, and the same as a QR code (SVG) for the phone. */
export type PairingInfo = { payload: string; qr_svg: string; expires_at: number; addrs: string[]; port: number };

/** A paired Device. Times are Unix seconds. */
export type Peer = {
  device_id: string;
  name: string;
  /** null for a Device that connects to this one. */
  last_addr: string | null;
  paired_at: number | null;
  last_sync_at: number | null;
};

export type SyncReport = {
  peer_device_id: string;
  sent: number;
  received: number;
  /** Synced entities (table names) the received changes touched. */
  entities: string[];
  converged: boolean;
};

export type SyncProgress = { peer_device_id: string; phase: "exchanging" | "finished"; sent: number; received: number };

export type JoinResult = { peer: Peer; report: SyncReport };

export const syncApi = {
  status: () => invoke<SyncStatus>("sync_status"),
  /** `firewall`: the answer to the firewall explanation (true adds the rule first; Windows asks for permission). */
  serverSet: (enabled: boolean, port: number, firewall?: boolean) =>
    invoke<SyncStatus>("sync_server_set", { enabled, port, firewall: firewall ?? null }),
  pairingStart: () => invoke<PairingInfo>("pairing_start"),
  pairingCancel: () => invoke<void>("pairing_cancel"),
  /** Pair with the Device whose payload was pasted, then run the first Sync. */
  pairingJoin: (payload: string) => invoke<JoinResult>("pairing_join", { payload }),
  peers: () => invoke<Peer[]>("peers_list"),
  removePeer: (deviceId: string) => invoke<void>("peer_remove", { deviceId }),
  now: (deviceId: string) => invoke<SyncReport>("sync_now", { deviceId }),
  /** The quick automatic Sync when the phone app comes to the foreground (about 1.5 s to connect). */
  auto: (deviceId: string) => invoke<SyncReport>("sync_auto", { deviceId }),
};
