import { ImageResponse } from "next/og";
import { SITE_NAME } from "@/lib/site";

export const alt = `${SITE_NAME} — trading journal, analytics and MCP for Claude`;
export const size = { width: 1200, height: 630 };
export const contentType = "image/png";

export default function OpengraphImage() {
  return new ImageResponse(
    <div
      style={{
        width: "100%",
        height: "100%",
        display: "flex",
        flexDirection: "column",
        justifyContent: "space-between",
        background: "#0A0A0B",
        padding: "72px",
      }}
    >
      <div
        style={{
          display: "flex",
          fontSize: 26,
          letterSpacing: "0.18em",
          textTransform: "uppercase",
          color: "#71717a",
        }}
      >
        {SITE_NAME}
      </div>

      <div
        style={{
          display: "flex",
          flexDirection: "column",
          fontSize: 68,
          fontWeight: 600,
          lineHeight: 1.1,
          letterSpacing: "-0.03em",
        }}
      >
        <span style={{ color: "#fafafa" }}>Your trades are a record.</span>
        <span style={{ color: "#52525b" }}>Most traders never read it.</span>
      </div>

      <div style={{ display: "flex", fontSize: 28, color: "#a1a1aa" }}>
        Broker sync · 36 analytics · Your journal, inside Claude
      </div>
    </div>,
    size,
  );
}
