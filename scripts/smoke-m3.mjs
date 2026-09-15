// M3 全流程冒烟：登录 mock → 搜索 → 购买 → 入队下载 → 状态 → 日志
const port = process.argv[2] || "9224";
const query = process.argv[3] || "wechat";

const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = list.find((t) => t.type === "page");
const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0;
const pending = new Map();

function call(expression) {
  return new Promise((resolve) => {
    const msgId = ++id;
    pending.set(msgId, resolve);
    ws.send(
      JSON.stringify({
        id: msgId,
        method: "Runtime.evaluate",
        params: { expression, returnByValue: true, awaitPromise: true },
      }),
    );
  });
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

ws.onmessage = (event) => {
  const msg = JSON.parse(event.data);
  if (msg.id && pending.has(msg.id)) {
    const resolve = pending.get(msg.id);
    pending.delete(msg.id);
    const value = msg.result?.result?.value;
    resolve(value);
  }
};

ws.onopen = async () => {
  try {
    // 1. mock 登录
    const login = await call(
      `window.__TAURI_INTERNALS__.invoke('auth_login', { account: 'test', password: 'test', passphrase: null }).then(r => r.status)`,
    );
    console.log("1. 登录:", login);

    // 2. 搜索（真实 iTunes API）
    await call(`window.__TAURI_INTERNALS__.invoke('catalog_search', { query: ${JSON.stringify(query)} }).then(r => { window.__results = r; return r.length; })`);
    const count = await call(`window.__results.length`);
    console.log("2. 搜索结果数:", count);

    // 3. 取第一个免费可购结果购买（mock 直通）
    const target = await call(`(() => {
      const r = window.__results.find(x => x.price === 'free') ?? window.__results[0];
      window.__target = r;
      return JSON.stringify({ bundleId: r.bundleId, name: r.name, price: r.price, purchased: r.purchased });
    })()`);
    console.log("3. 目标:", target);
    const targetObj = JSON.parse(target);
    const purchase = await call(
      `window.__TAURI_INTERNALS__.invoke('purchase', { bundleId: ${JSON.stringify(targetObj.bundleId)}, price: ${JSON.stringify(targetObj.price)}, purchased: ${JSON.stringify(targetObj.purchased)} }).then(r => r.outcome + '/' + (r.detail ?? ''))`,
    );
    console.log("4. 购买:", purchase);

    // 4. 入队 + 启动（mock 直通成功）
    const add = await call(
      `window.__TAURI_INTERNALS__.invoke('queue_add', { bundleId: ${JSON.stringify(targetObj.bundleId)}, appId: window.__target.id, name: window.__target.name, developer: window.__target.developer, version: window.__target.version, price: window.__target.price, artworkUrl: window.__target.artworkUrl })`,
    );
    console.log("5. 入队:", add);
    const startErr = await call(`window.__TAURI_INTERNALS__.invoke('queue_start').then(() => 'ok').catch(e => String(e))`);
    console.log("6. 启动:", startErr);

    // 5. 等 mock 队列跑完，查状态
    await sleep(2500);
    const status = await call(
      `window.__TAURI_INTERNALS__.invoke('queue_status').then(s => JSON.stringify({ running: s.running, items: s.items.map(i => i.bundleId + '=' + i.status) }))`,
    );
    console.log("7. 队列状态:", status);

    // 6. 日志
    const logs = await call(
      `window.__TAURI_INTERNALS__.invoke('logs_snapshot').then(l => l.length + ' 条, 末条: ' + JSON.stringify(l[l.length - 1] ?? null).slice(0, 160))`,
    );
    console.log("8. 日志:", logs);
    process.exit(0);
  } catch (e) {
    console.error("失败:", e);
    process.exit(1);
  }
};
