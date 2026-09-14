// CDP 页面检查器：执行 JS 表达式并打印结果
// 用法: node scripts/eval-webview.mjs <port> <expression>
const port = process.argv[2] || "9224";
const expression = process.argv[3] || "document.body.innerText";

const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = list.find((t) => t.type === "page");
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
