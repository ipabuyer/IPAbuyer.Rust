// CDP 前端错误监听器：连接 WebView2 远程调试端口，把页面 console/异常转发到终端
// 用法: node scripts/watch-webview-errors.mjs [port] [--timeout 秒]
const port = process.argv[2] || "9222";
const timeoutMs = (process.argv.includes("--timeout")
  ? Number(process.argv[process.argv.indexOf("--timeout") + 1])
  : 0) * 1000;

const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const pages = list.filter((t) => t.type === "page");
if (pages.length === 0) {
  console.error("未找到页面目标");
  process.exit(1);
}
const ws = new WebSocket(pages[0].webSocketDebuggerUrl);
let count = 0;

ws.onopen = () => {
  console.log(`已连接 ${pages[0].url}`);
  ws.send(JSON.stringify({ id: 1, method: "Runtime.enable" }));
  ws.send(JSON.stringify({ id: 2, method: "Log.enable" }));
  if (timeoutMs) setTimeout(() => process.exit(0), timeoutMs);
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.method === "Runtime.exceptionThrown") {
    const d = msg.params.exceptionDetails;
    console.error(`[异常] ${d.text} ${d.exception?.description ?? ""}`);
    count++;
  } else if (msg.method === "Runtime.consoleAPICalled") {
    const { type, args } = msg.params;
    if (type === "error" || type === "warning") {
      console.error(`[console.${type}]`, args.map((a) => a.value ?? a.description ?? "").join(" "));
      count++;
    }
  } else if (msg.method === "Log.entryAdded") {
    const e = msg.params.entry;
    if (e.level === "error") {
      console.error(`[log.${e.level}] ${e.text} (${e.url ?? ""})`);
      count++;
    }
  }
};

ws.onerror = (e) => console.error("ws 错误", e.message);
ws.onclose = () => console.log(`连接关闭，共捕获 ${count} 条`);
