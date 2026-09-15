// CDP 页面截图：执行可选前置表达式后捕获视口
// 用法: node scripts/screenshot.mjs <port> <outfile.png> [expression] [windowLabel]
// windowLabel 缺省时取第一个非 log 窗口（CDP 列表顺序不稳定，隐藏的日志窗口可能排前）
const port = process.argv[2] || "9224";
const outfile = process.argv[3] || "screenshot.png";
const expression = process.argv[4];
const wantLabel = process.argv[5];

const list = (await (await fetch(`http://127.0.0.1:${port}/json/list`)).json()).filter(
  (t) => t.type === "page",
);

function probeLabel(target) {
  return new Promise((resolve) => {
    const ws = new WebSocket(target.webSocketDebuggerUrl);
    ws.onopen = () =>
      ws.send(
        JSON.stringify({
          id: 1,
          method: "Runtime.evaluate",
          params: {
            expression: "window.__TAURI_INTERNALS__?.metadata?.currentWindow?.label ?? ''",
            returnByValue: true,
          },
        }),
      );
    ws.onmessage = (e) => {
      const m = JSON.parse(e.data);
      if (m.id === 1) {
        const label = m.result?.result?.value ?? "";
        ws.close();
        resolve(label);
      }
    };
    setTimeout(() => resolve(""), 2000);
  });
}

let target = list[0];
if (list.length > 1 || wantLabel !== undefined) {
  for (const t of list) {
    const label = await probeLabel(t);
    if (wantLabel !== undefined ? label === wantLabel : label !== "log") {
      target = t;
      break;
    }
  }
}

const { writeFileSync } = await import("node:fs");
const data = await new Promise((resolve, reject) => {
  const ws = new WebSocket(target.webSocketDebuggerUrl);
  ws.onopen = async () => {
    await ws.send(JSON.stringify({ id: 1, method: "Page.enable" }));
    if (expression) {
      ws.send(JSON.stringify({ id: 2, method: "Runtime.evaluate", params: { expression, returnByValue: true } }));
      await new Promise((r) => setTimeout(r, 600));
    }
    ws.send(JSON.stringify({ id: 3, method: "Page.captureScreenshot", params: { format: "png" } }));
  };
  ws.onmessage = (e) => {
    const m = JSON.parse(e.data);
    if (m.id === 3 && m.result?.data) resolve(m.result.data);
  };
  setTimeout(() => reject(new Error("capture timeout")), 10000);
});
writeFileSync(outfile, Buffer.from(data, "base64"));
console.log(`saved: ${outfile}`);
