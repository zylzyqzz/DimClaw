const panel = document.getElementById("panel");
const titleEl = document.getElementById("page-title");
const subEl = document.getElementById("page-sub");
const serviceStatus = document.getElementById("service-status");

const TAB_META = {
  chat: { title: "对话(Chat)", sub: "打开即对话，支持技能调用与多智能体回复" },
  models: { title: "模型配置(Model Config)", sub: "模型列表、默认模型与连接状态" },
  channels: { title: "通道配置(Channel Config)", sub: "飞书与 Telegram 配置、状态与智能体映射" },
  agents: { title: "智能体配置(Agent Config)", sub: "基础智能体与自定义智能体集中管理" },
  skills: { title: "技能管理(Skill Manager)", sub: "技能列表、测试与导入" },
  marketplace: { title: "技能市场(Marketplace)", sub: "分类浏览、搜索与安装技能" },
  hands: { title: "自主任务(Hands)", sub: "查看、触发、暂停与恢复自主任务" },
  plugins: { title: "插件管理(Plugin Manager)", sub: "插件安装、启用、禁用与卸载" },
  audit: { title: "审计日志(Audit)", sub: "查看近期系统与技能执行日志" },
  system: { title: "系统设置(System)", sub: "安全与运行时配置" },
};

const state = {
  tab: "chat",
  chatHistory: JSON.parse(localStorage.getItem("dimclaw-chat") || "[]"),
  channel: "",
};

function esc(v) {
  return String(v ?? "")
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

async function request(url, method = "GET", body) {
  const resp = await fetch(url, {
    method,
    headers: body ? { "Content-Type": "application/json" } : undefined,
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await resp.text();
  let json = {};
  if (text.trim()) {
    try { json = JSON.parse(text); } catch { json = { error: text }; }
  }
  if (!resp.ok) throw new Error(json.error || text || `HTTP ${resp.status}`);
  return json;
}

function persistableHistory() {
  return state.chatHistory.filter((x) => !x.temp).slice(-80);
}

function saveChatHistory() {
  localStorage.setItem("dimclaw-chat", JSON.stringify(persistableHistory()));
}

function pushChat(role, text, agent = "", extra = {}) {
  state.chatHistory.push({ role, text, agent, ts: new Date().toISOString(), ...extra });
  state.chatHistory = state.chatHistory.slice(-120);
  saveChatHistory();
}

function fmtTime(ts) {
  const d = new Date(ts || Date.now());
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleTimeString();
}

function setNavActive() {
  document.querySelectorAll("#nav button").forEach((btn) => btn.classList.toggle("active", btn.dataset.tab === state.tab));
  titleEl.textContent = TAB_META[state.tab].title;
  subEl.textContent = TAB_META[state.tab].sub;
}

async function renderServiceStatus() {
  try {
    const stats = await request("/api/dashboard/stats");
    const conn = await request("/api/status/connections");
    const dotCls = (conn.model || {}).status === "connected" ? "ok" : "";
    serviceStatus.innerHTML = `<span class="dot ${dotCls}"></span><span>服务运行(Service): ${esc(stats.uptime_secs)}s</span>`;
  } catch {
    serviceStatus.innerHTML = `<span class="dot"></span><span>服务异常(Service Error)</span>`;
  }
}

function renderChat() {
  const messages = state.chatHistory.map((m) => {
    const cls = m.role === "user" ? "user" : "assistant";
    const who = m.role === "user" ? "你(User)" : `助手(Assistant)${m.agent ? ` - ${m.agent}` : ""}`;
    const bubbleCls = `${cls}${m.thinking ? " thinking" : ""}${m.error ? " error" : ""}`;
    const tool = m.tool_summary
      ? `<div class="note" style="margin-top:6px;">工具调用(Tool): ${esc(JSON.stringify(m.tool_summary))}</div>`
      : "";
    return `
      <div class="msg-row ${cls}">
        <div class="msg-bubble ${bubbleCls}">
          <div class="msg-meta">${esc(who)} · ${esc(fmtTime(m.ts))}</div>
          <div>${esc(m.text)}</div>
          ${tool}
        </div>
      </div>`;
  }).join("");

  panel.innerHTML = `
    <div class="chat-shell">
      <div class="chat-messages" id="chat-messages">${messages || '<div class="note">暂无消息(No Messages)</div>'}</div>
      <div class="chat-input">
        <div class="grid">
          <div class="row">
            <label>通道路由(Channel)</label>
            <select id="chat-channel">
              <option value="">本地对话(Local)</option>
              <option value="feishu" ${state.channel === "feishu" ? "selected" : ""}>飞书(Feishu)</option>
              <option value="telegram" ${state.channel === "telegram" ? "selected" : ""}>Telegram(电报)</option>
            </select>
          </div>
          <textarea id="chat-input-text" placeholder="输入消息，例如：创建一个文件 test.txt 内容为 hello"></textarea>
        </div>
        <div class="chat-actions">
          <button class="btn primary" id="chat-send">发送(Send)</button>
          <button class="btn" id="chat-clear">清空(Clear)</button>
        </div>
      </div>
    </div>`;

  const list = document.getElementById("chat-messages");
  list.scrollTop = list.scrollHeight;

  document.getElementById("chat-channel").addEventListener("change", (e) => { state.channel = e.target.value; });

  document.getElementById("chat-send").addEventListener("click", async () => {
    const input = document.getElementById("chat-input-text");
    const msg = input.value.trim();
    if (!msg) return;
    input.value = "";

    pushChat("user", msg);
    const marker = `thinking-${Date.now()}-${Math.random()}`;
    pushChat("assistant", "智能体思考中...", "System", { thinking: true, temp: true, marker });
    renderChat();

    try {
      const out = await request("/api/chat", "POST", {
        message: msg,
        history: persistableHistory().slice(-20).map((x) => ({ role: x.role, content: x.text })),
        channel: state.channel,
      });
      const idx = state.chatHistory.findIndex((m) => m.marker === marker);
      if (idx >= 0) {
        state.chatHistory[idx] = {
          role: "assistant",
          text: out.reply || "",
          agent: out.agent_name || "",
          ts: new Date().toISOString(),
          tool_summary: out.tool_summary || null,
        };
      } else {
        pushChat("assistant", out.reply || "", out.agent_name || "");
      }
      saveChatHistory();
    } catch (e) {
      const idx = state.chatHistory.findIndex((m) => m.marker === marker);
      const text = `请求失败，请重试: ${String(e)}`;
      if (idx >= 0) {
        state.chatHistory[idx] = { role: "assistant", text, agent: "System(系统)", ts: new Date().toISOString(), error: true };
      } else {
        pushChat("assistant", text, "System(系统)", { error: true });
      }
      saveChatHistory();
    }
    renderChat();
  });

  document.getElementById("chat-clear").addEventListener("click", () => {
    state.chatHistory = [];
    saveChatHistory();
    renderChat();
  });

  document.getElementById("chat-input-text").addEventListener("keydown", (e) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      document.getElementById("chat-send").click();
    }
  });
}

async function renderModels() {
  const models = await request("/api/config/models");
  const conn = await request("/api/status/connections").catch(() => ({ model: {} }));
  const m = conn.model || {};
  panel.innerHTML = `
    <div class="card">
      <h3>连接状态(Connection Status)</h3>
      <div class="row"><span class="label">当前模型(Current Model)</span><span>${esc(m.provider || "-")}</span></div>
      <div class="row"><span class="label">状态(Status)</span><span class="status-pill ${m.status === "connected" ? "ok" : ""}">${esc(m.status || "unknown")}</span></div>
      <div class="row"><span class="label">说明(Message)</span><span>${esc(m.message || "-")}</span></div>
    </div>
    <div class="card"><h3>模型列表(Model List)</h3>
      <table class="table"><thead><tr><th>名称</th><th>模型</th><th>默认</th><th>启用</th><th>操作</th></tr></thead><tbody>
      ${(models.providers || []).map((p) => `<tr><td>${esc(p.name)}</td><td>${esc(p.model)}</td><td>${p.default ? "是" : "否"}</td><td>${p.enabled ? "是" : "否"}</td><td><button class="btn" data-m-default="${esc(p.name)}">设默认</button></td></tr>`).join("")}
      </tbody></table>
    </div>`;
  panel.querySelectorAll("[data-m-default]").forEach((btn) => btn.addEventListener("click", async () => {
    await request("/api/config/models/default", "POST", { name: btn.dataset.mDefault });
    await renderModels();
  }));
}

function channelCard(name, cfg, plugin) {
  const title = name === "feishu" ? "飞书(Feishu)" : "Telegram(电报)";
  const status = plugin ? `${plugin.running ? "运行中" : "已停止"} / ${plugin.connection_status || "disconnected"}` : "未安装";
  return `<div class="card">
    <h3>${title}</h3>
    <div class="row"><span class="label">插件状态</span><span>${esc(status)}</span></div>
    <div class="row"><label>插件已安装(Installed)</label><input id="ch_${name}_installed" type="checkbox" ${cfg.plugin_installed ? "checked" : ""}></div>
    <div class="row"><label>启用(Enabled)</label><input id="ch_${name}_enabled" type="checkbox" ${cfg.enabled ? "checked" : ""}></div>
    <div class="row"><label>默认智能体(Default Agent)</label><input id="ch_${name}_default" value="${esc(cfg.default_agent || "Planner")}"></div>
    <div class="actions"><button class="btn primary" data-ch-save="${name}">保存</button><button class="btn" data-ch-detail="${name}">详情配置</button></div>
  </div>`;
}

async function renderChannels() {
  const channels = await request("/api/config/channels");
  const plugins = await request("/api/plugins").catch(() => []);
  const fei = (plugins || []).find((p) => p.name === "feishu");
  const tg = (plugins || []).find((p) => p.name === "telegram");
  panel.innerHTML = `${channelCard("feishu", channels.feishu || {}, fei)}${channelCard("telegram", channels.telegram || {}, tg)}`;

  panel.querySelectorAll("[data-ch-save]").forEach((btn) => btn.addEventListener("click", async () => {
    const n = btn.dataset.chSave;
    const prev = await request(`/api/config/channels/${n}`).catch(() => ({}));
    const payload = {
      ...prev,
      plugin_installed: document.getElementById(`ch_${n}_installed`).checked,
      enabled: document.getElementById(`ch_${n}_enabled`).checked,
      default_agent: document.getElementById(`ch_${n}_default`).value.trim() || "Planner",
      single_agent: prev.single_agent || "Planner",
      mode: prev.mode || "single",
      agents: prev.agents || ["Planner", "Executor", "Verifier", "Recovery"],
      agent_map: prev.agent_map || {},
    };
    await request(`/api/config/channels/${n}`, "PUT", payload);
    alert("保存成功");
  }));

  panel.querySelectorAll("[data-ch-detail]").forEach((btn) => btn.addEventListener("click", async () => {
    await renderChannelDetail(btn.dataset.chDetail);
  }));
}

async function renderChannelDetail(name) {
  const ch = await request(`/api/config/channels/${name}`);
  const custom = await request("/api/agents/custom").catch(() => []);
  const names = ["Planner", "Executor", "Verifier", "Recovery", ...(custom || []).map((v) => v.name)];

  panel.innerHTML = `<div class="card">
    <h3>通道详情(Channel Detail): ${esc(name)}</h3>
    <div class="row"><label>启用(Enabled)</label><input id="cd_enabled" type="checkbox" ${ch.enabled ? "checked" : ""}></div>
    <div class="row"><label>默认智能体(Default Agent)</label><input id="cd_default" value="${esc(ch.default_agent || "Planner")}"></div>
    ${name === "feishu"
      ? `<div class="row"><label>app_id</label><input id="cd_app_id" value="${esc(ch.app_id || "")}"></div>
         <div class="row"><label>app_secret</label><input id="cd_app_secret" value="${esc(ch.app_secret || "")}"></div>
         <div class="row"><label>verify_token</label><input id="cd_verify_token" value="${esc(ch.verify_token || "")}"></div>`
      : `<div class="row"><label>bot_token</label><input id="cd_bot_token" value="${esc(ch.bot_token || "")}"></div>`}
  </div>
  <div class="card">
    <h3>智能体映射(Agent Map)</h3>
    <table class="table"><thead><tr><th>名称</th><th>启用</th><th>关键词</th><th>风格</th></tr></thead><tbody>
      ${names.map((agent) => {
        const row = (ch.agent_map || {})[agent] || { enabled: true, keywords: [], style: "" };
        return `<tr>
          <td>${esc(agent)}</td>
          <td><input type="checkbox" id="am_en_${esc(agent)}" ${row.enabled ? "checked" : ""}></td>
          <td><input id="am_kw_${esc(agent)}" value="${esc((row.keywords || []).join(","))}"></td>
          <td><input id="am_st_${esc(agent)}" value="${esc(row.style || "")}"></td>
        </tr>`;
      }).join("")}
    </tbody></table>
    <div class="actions"><button class="btn primary" id="cd_save">保存通道详情</button><button class="btn" id="cd_back">返回</button></div>
  </div>`;

  document.getElementById("cd_back").addEventListener("click", renderChannels);
  document.getElementById("cd_save").addEventListener("click", async () => {
    const agent_map = {};
    names.forEach((agent) => {
      agent_map[agent] = {
        enabled: document.getElementById(`am_en_${agent}`).checked,
        keywords: document.getElementById(`am_kw_${agent}`).value.split(",").map((s) => s.trim()).filter(Boolean),
        style: document.getElementById(`am_st_${agent}`).value.trim(),
      };
    });
    const payload = {
      ...ch,
      enabled: document.getElementById("cd_enabled").checked,
      default_agent: document.getElementById("cd_default").value.trim() || "Planner",
      agent_map,
      app_id: document.getElementById("cd_app_id")?.value || ch.app_id || "",
      app_secret: document.getElementById("cd_app_secret")?.value || ch.app_secret || "",
      verify_token: document.getElementById("cd_verify_token")?.value || ch.verify_token || "",
      bot_token: document.getElementById("cd_bot_token")?.value || ch.bot_token || "",
    };
    await request(`/api/config/channels/${name}`, "PUT", payload);
    alert("保存成功");
  });
}

async function renderAgents() {
  const base = await request("/api/config/agents").catch(() => ({}));
  const custom = await request("/api/agents/custom").catch(() => []);
  panel.innerHTML = `
    <div class="card"><h3>基础智能体</h3><table class="table"><tbody>
      <tr><td>Planner</td><td>${esc((base.planner || {}).system_prompt || "-")}</td></tr>
      <tr><td>Executor</td><td>${esc((base.executor || {}).system_prompt || "-")}</td></tr>
      <tr><td>Verifier</td><td>${esc((base.verifier || {}).system_prompt || "-")}</td></tr>
      <tr><td>Recovery</td><td>${esc((base.recovery || {}).system_prompt || "-")}</td></tr>
    </tbody></table></div>
    <div class="card">
      <h3>自定义智能体(Custom Agents)</h3>
      <div class="grid">
        <div class="row"><label>名称</label><input id="ag_name"></div>
        <div class="row"><label>描述</label><input id="ag_desc"></div>
        <div class="row"><label>角色</label><input id="ag_role"></div>
        <div class="row"><label>阶段</label><input id="ag_phase" value="after_planning"></div>
        <div class="row"><label>模型</label><input id="ag_model"></div>
        <div class="row"><label>关键词</label><input id="ag_kw"></div>
        <div class="row"><label>提示词</label><textarea id="ag_prompt"></textarea></div>
        <div class="actions"><button class="btn primary" id="ag_create">创建</button></div>
      </div>
      <table class="table" style="margin-top:10px"><thead><tr><th>名称</th><th>阶段</th><th>模型</th><th>操作</th></tr></thead><tbody>
        ${(custom || []).map((a) => `<tr><td>${esc(a.name)}</td><td>${esc(a.phase)}</td><td>${esc(a.model || "")}</td><td><button class="btn danger" data-ag-del="${esc(a.name)}">删除</button></td></tr>`).join("")}
      </tbody></table>
    </div>`;

  document.getElementById("ag_create").addEventListener("click", async () => {
    await request("/api/agents/custom", "POST", {
      name: document.getElementById("ag_name").value.trim(),
      description: document.getElementById("ag_desc").value.trim(),
      role: document.getElementById("ag_role").value.trim(),
      phase: document.getElementById("ag_phase").value.trim() || "after_planning",
      model: document.getElementById("ag_model").value.trim(),
      trigger_keywords: document.getElementById("ag_kw").value.split(",").map((s) => s.trim()).filter(Boolean),
      system_prompt_template: document.getElementById("ag_prompt").value,
      enabled: true,
    });
    await renderAgents();
  });

  panel.querySelectorAll("[data-ag-del]").forEach((btn) => btn.addEventListener("click", async () => {
    await request(`/api/agents/custom/${encodeURIComponent(btn.dataset.agDel)}`, "DELETE");
    await renderAgents();
  }));
}

async function renderSkills() {
  const list = await request("/api/skills").catch(() => []);
  panel.innerHTML = `
    <div class="card">
      <h3>导入技能(Import Skill)</h3>
      <div class="row"><label>技能 JSON</label><textarea id="sk_json" placeholder='{"name":"openclaw_echo","command":"echo {{msg}}"}'></textarea></div>
      <div class="actions"><button class="btn primary" id="sk_import">导入技能</button></div>
    </div>
    <div class="card"><h3>技能列表</h3>
      <table class="table"><thead><tr><th>名称</th><th>类型</th><th>来源</th><th>操作</th></tr></thead><tbody>
      ${(list || []).map((s) => `<tr><td>${esc(s.name)}</td><td>${esc(s.exec_type)}</td><td>${s.builtin ? "内置" : "自定义"}</td><td><button class="btn" data-sk-test="${esc(s.name)}">测试</button></td></tr>`).join("")}
      </tbody></table>
    </div>`;

  document.getElementById("sk_import").addEventListener("click", async () => {
    const skillObj = JSON.parse(document.getElementById("sk_json").value || "{}");
    await request("/api/skills/openclaw/import", "POST", { skill: skillObj, overwrite: true });
    await renderSkills();
  });
  panel.querySelectorAll("[data-sk-test]").forEach((btn) => btn.addEventListener("click", async () => {
    const raw = prompt("输入测试参数 JSON", "{}");
    if (raw == null) return;
    const out = await request(`/api/skills/${encodeURIComponent(btn.dataset.skTest)}/test`, "POST", { input: JSON.parse(raw || "{}"), timeout_secs: 20 });
    alert(JSON.stringify(out, null, 2));
  }));
}

async function renderMarketplace() {
  const data = await request("/api/marketplace");
  panel.innerHTML = `
    <div class="card">
      <h3>技能市场(Marketplace)</h3>
      <div class="row"><label>搜索</label><input id="mk_q" placeholder="例如：邮件自动化"></div>
      <div class="actions"><button class="btn" id="mk_search">搜索</button><button class="btn primary" id="mk_import">在线批量导入</button></div>
    </div>
    <div class="card"><h3>分类(Categories)</h3>${(data.categories || []).map((c) => `<div class="note">${esc(c[0])}: ${esc((c[1] || []).join(", "))}</div>`).join("")}</div>
    <div class="card"><h3>技能列表</h3><table class="table"><thead><tr><th>名称</th><th>描述</th><th>分类</th><th>作者</th><th>下载</th><th>星级</th><th>操作</th></tr></thead><tbody>
      ${(data.skills || []).map((s) => `<tr><td>${esc(s.name)}</td><td>${esc(s.description)}</td><td>${esc(s.category)}</td><td>${esc(s.author)}</td><td>${esc(s.downloads)}</td><td>${esc(s.stars)}</td><td><button class="btn primary" data-mk-install="${esc(s.name)}">安装</button></td></tr>`).join("")}
    </tbody></table></div>`;

  document.getElementById("mk_search").addEventListener("click", async () => {
    const q = document.getElementById("mk_q").value.trim();
    const re = await request(`/api/marketplace?q=${encodeURIComponent(q)}`);
    data.skills = re.skills || [];
    renderMarketplaceWithData(re);
  });
  document.getElementById("mk_import").addEventListener("click", async () => {
    const repo_url = prompt("输入仓库 URL", "https://github.com/VoltAgent/awesome-openclaw-skills.git") || "";
    const out = await request("/api/marketplace/import", "POST", { repo_url });
    alert(JSON.stringify(out, null, 2));
  });
  panel.querySelectorAll("[data-mk-install]").forEach((btn) => btn.addEventListener("click", async () => {
    await request(`/api/marketplace/install/${encodeURIComponent(btn.dataset.mkInstall)}`, "POST", {});
    alert("安装成功");
  }));
}

function renderMarketplaceWithData(data) {
  panel.innerHTML = `
    <div class="card">
      <h3>技能市场(Marketplace)</h3>
      <div class="actions"><button class="btn" id="mk_back">返回</button></div>
    </div>
    <div class="card"><table class="table"><thead><tr><th>名称</th><th>描述</th><th>分类</th><th>操作</th></tr></thead><tbody>
      ${(data.skills || []).map((s) => `<tr><td>${esc(s.name)}</td><td>${esc(s.description)}</td><td>${esc(s.category)}</td><td><button class="btn primary" data-mk-install="${esc(s.name)}">安装</button></td></tr>`).join("")}
    </tbody></table></div>`;
  document.getElementById("mk_back").addEventListener("click", renderMarketplace);
  panel.querySelectorAll("[data-mk-install]").forEach((btn) => btn.addEventListener("click", async () => {
    await request(`/api/marketplace/install/${encodeURIComponent(btn.dataset.mkInstall)}`, "POST", {});
    alert("安装成功");
  }));
}

async function renderHands() {
  const list = await request("/api/hands").catch(() => []);
  panel.innerHTML = `<div class="card"><h3>Hands 列表</h3>
    <table class="table"><thead><tr><th>名称</th><th>描述</th><th>上次</th><th>下次</th><th>状态</th><th>结果</th><th>操作</th></tr></thead><tbody>
      ${(list || []).map((h) => `<tr>
        <td>${esc(h.name)}</td><td>${esc(h.description)}</td><td>${esc(h.last_run || "-")}</td><td>${esc(h.next_run || "-")}</td>
        <td>${h.paused ? "暂停" : "运行中"}</td><td>${esc(h.last_output || "-")}</td>
        <td><button class="btn" data-h-trigger="${esc(h.name)}">立即触发</button> <button class="btn" data-h-pause="${esc(h.name)}">暂停</button> <button class="btn" data-h-resume="${esc(h.name)}">恢复</button></td>
      </tr>`).join("")}
    </tbody></table></div>`;

  panel.querySelectorAll("[data-h-trigger]").forEach((btn) => btn.addEventListener("click", async () => { await request(`/api/hands/trigger/${encodeURIComponent(btn.dataset.hTrigger)}`, "POST", {}); await renderHands(); }));
  panel.querySelectorAll("[data-h-pause]").forEach((btn) => btn.addEventListener("click", async () => { await request(`/api/hands/pause/${encodeURIComponent(btn.dataset.hPause)}`, "POST", {}); await renderHands(); }));
  panel.querySelectorAll("[data-h-resume]").forEach((btn) => btn.addEventListener("click", async () => { await request(`/api/hands/resume/${encodeURIComponent(btn.dataset.hResume)}`, "POST", {}); await renderHands(); }));
}

function renderPluginCard(item) {
  return `<div class="card"><h3>${esc(item.name)} ${item.installed ? "(已安装)" : "(未安装)"}</h3>
    <div class="row"><span class="label">运行状态</span><span>${item.running ? "运行中" : "停止"}</span></div>
    <div class="row"><span class="label">连接状态</span><span>${esc(item.connection_status || "-")}</span></div>
    <div class="actions">
      <button class="btn primary" data-pl-install="${esc(item.name)}">安装</button>
      <button class="btn" data-pl-enable="${esc(item.name)}">启用</button>
      <button class="btn" data-pl-disable="${esc(item.name)}">禁用</button>
      <button class="btn danger" data-pl-uninstall="${esc(item.name)}">卸载</button>
    </div></div>`;
}

async function renderPlugins() {
  const available = await request("/api/plugins/available").catch(() => []);
  const installed = await request("/api/plugins").catch(() => []);
  const merged = (available || []).map((a) => ({ ...a, ...((installed || []).find((x) => x.name === a.name) || {}) }));
  panel.innerHTML = merged.map(renderPluginCard).join("") || '<div class="card">暂无插件</div>';

  panel.querySelectorAll("[data-pl-install]").forEach((btn) => btn.addEventListener("click", async () => {
    const name = btn.dataset.plInstall;
    if (name === "feishu") {
      await request(`/api/plugins/install/${name}`, "POST", { app_id: prompt("app_id", "") || "", app_secret: prompt("app_secret", "") || "", verify_token: prompt("verify_token", "") || "" });
    } else if (name === "telegram") {
      await request(`/api/plugins/install/${name}`, "POST", { bot_token: prompt("bot_token", "") || "" });
    } else {
      await request(`/api/plugins/install/${name}`, "POST", {});
    }
    await renderPlugins();
  }));
  panel.querySelectorAll("[data-pl-enable]").forEach((btn) => btn.addEventListener("click", async () => { await request(`/api/plugins/enable/${btn.dataset.plEnable}`, "POST", {}); await renderPlugins(); }));
  panel.querySelectorAll("[data-pl-disable]").forEach((btn) => btn.addEventListener("click", async () => { await request(`/api/plugins/disable/${btn.dataset.plDisable}`, "POST", {}); await renderPlugins(); }));
  panel.querySelectorAll("[data-pl-uninstall]").forEach((btn) => btn.addEventListener("click", async () => { await request(`/api/plugins/uninstall/${btn.dataset.plUninstall}`, "POST", {}); await renderPlugins(); }));
}

async function renderAudit() {
  const logs = await request("/api/logs/recent").catch(() => ({ lines: [] }));
  panel.innerHTML = `<div class="card"><h3>审计日志(Audit Logs)</h3><pre style="max-height:520px;overflow:auto;white-space:pre-wrap;">${esc((logs.lines || []).join("\n"))}</pre></div>`;
}

async function renderSystem() {
  const sec = await request("/api/config/security").catch(() => ({ unrestricted_mode: false }));
  panel.innerHTML = `<div class="card"><h3>安全设置(Security)</h3>
    <div class="warn-box">开启无限制模式后，智能体可访问任意路径并执行任意命令，请谨慎操作。</div>
    <div class="row" style="margin-top:10px"><label>无限制模式</label><input id="sec_unrestricted" type="checkbox" ${sec.unrestricted_mode ? "checked" : ""}></div>
    <div class="actions"><button class="btn primary" id="sec_save">保存</button></div>
  </div>`;
  document.getElementById("sec_save").addEventListener("click", async () => {
    await request("/api/config/security", "PUT", { unrestricted_mode: document.getElementById("sec_unrestricted").checked });
    alert("保存成功");
  });
}

async function renderTab() {
  if (state.tab === "chat") return renderChat();
  if (state.tab === "models") return renderModels();
  if (state.tab === "channels") return renderChannels();
  if (state.tab === "agents") return renderAgents();
  if (state.tab === "skills") return renderSkills();
  if (state.tab === "marketplace") return renderMarketplace();
  if (state.tab === "hands") return renderHands();
  if (state.tab === "plugins") return renderPlugins();
  if (state.tab === "audit") return renderAudit();
  if (state.tab === "system") return renderSystem();
}

function bindNav() {
  document.querySelectorAll("#nav button").forEach((btn) => btn.addEventListener("click", async () => {
    state.tab = btn.dataset.tab;
    setNavActive();
    await renderTab();
  }));
}

async function bootstrap() {
  bindNav();
  setNavActive();
  await renderServiceStatus();
  await renderTab();
  setInterval(renderServiceStatus, 5000);
}

bootstrap();
