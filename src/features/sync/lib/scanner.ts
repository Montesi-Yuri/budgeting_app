// The phone's camera scanner for the PC's pairing QR code (ticket 15).
// The barcode-scanner plugin exists only in the phone build, so this is
// called only when `AppInfo.mobile` is true.
import { Format, requestPermissions, scan } from "@tauri-apps/plugin-barcode-scanner";
import { scannedPairingCode, Scanned } from "./pairing";

/** Ask for the camera if needed, scan one QR code and check it is a pairing code. */
export async function scanPairingCode(): Promise<Scanned> {
  const permission = await requestPermissions();
  if (permission !== "granted") {
    return {
      ok: false,
      error: "The app needs the camera to scan the pairing code. Allow it in the phone's settings, or paste the code instead.",
    };
  }
  const scanned = await scan({ windowed: false, formats: [Format.QRCode] });
  return scannedPairingCode(scanned.content);
}
