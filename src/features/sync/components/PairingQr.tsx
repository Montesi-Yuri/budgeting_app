/** The pairing code as a QR code (an SVG made by the shell), for the phone's camera. */
export default function PairingQr({ svg }: { svg: string }) {
  return (
    <img
      className="pairing-qr"
      alt="Pairing QR code: scan it with the phone"
      src={`data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`}
    />
  )
}
