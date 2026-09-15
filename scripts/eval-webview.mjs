// CDP 页面检查器：执行 JS 表达式并打印结果
// 用法: node scripts/eval-webview.mjs <port> <expression> [windowLabel]
// windowLabel 缺省时取第一个非 log 窗口（多窗口下 CDP 列表顺序不稳定）
const port = process.argv[2] || "9224";
const expression = process.argv[3] || "document.body.innerText";
const wantLabel = process.argv[4];

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

let page = list[0];
if (list.length > 1 || wantLabel !== undefined) {
  for (const t of list) {
    const label = await probeLabel(t);
    if (wantLabel !== undefined ? label === wantLabel : label !== "log") {
      page = t;
      break;
    }
  }
}

const ws = new WebSocket(page.webSocketDebuggerUrl);

ws.onopen = () => {
  ws.send(
    JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression, returnByValue: true, awaitPromise: true } }),
  );
};
ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id === 1) {
    const r = msg.result?.result;
    console.log(r?.value ?? JSON.stringify(msg.result));
    process.exit(0);
  }
};
