// 在页面新文档执行前注入脚本并刷新，验证注入→i18n 链路
// 用法: node scripts/test-injection.mjs <port>
const port = process.argv[2] || "9224";

const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = list.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
let step = 0;

function send(id, method, params) {
  ws.send(JSON.stringify({ id, method, params: params ?? {} }));
}

ws.onopen = () => {
  send(1, "Page.enable");
  send(2, "Page.addScriptToEvaluateOnNewDocument", {
    source: 'window.__IPABUYER_LANG__ = "en-US";',
  });
  send(3, "Page.reload");
};

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id === 3 && msg.result) {
    // 等 reload 完成后检查语言
    setTimeout(async () => {
      send(4, "Runtime.evaluate", {
        expression:
          "JSON.stringify({ injected: window.__IPABUYER_LANG__, text: document.body.innerText.replace(/\\n/g, '|').slice(0, 120) })",
        returnByValue: true,
        awaitPromise: true,
      });
    }, 3000);
  }
  if (msg.id === 4) {
    console.log(msg.result?.result?.value);
    process.exit(0);
  }
};
