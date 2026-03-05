const wizard = document.getElementById("wizard");
const dashboard = document.getElementById("dashboard");

async function getJson(url) {
  const r = await fetch(url);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

async function putJson(url, body) {
  const r = await fetch(url, {
    method: "PUT",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body)
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

async function postJson(url, body) {
  const r = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body)
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

async function init() {
  const models = await getJson("/api/config/models");
  if (!models.providers || models.providers.length === 0 || location.pathname === "/wizard") {
    renderWizard();
  } else {
    renderDashboard();
  }
}

function renderWizard() {
  dashboard.classList.add("hidden");
  wizard.classList.remove("hidden");
  wizard.innerHTML = `
    <h2>步骤1: 欢迎</h2>
    <p>DimClaw 将引导你配置模型、飞书通道与智能体提示词。</p>
    <h2>步骤2: 添加模型提供者</h2>
    <label>名称</label><input id="m_name" value="default" />
    <label>协议</label><input id="m_protocol" value="openai_compatible" />
    <label>base_url</label><input id="m_base" value="https://integrate.api.nvidia.com/v1" />
    <label>api_key</label><input id="m_key" value="" />
    <label>model</label><input id="m_model" value="nvidia/qwen/qwen3.5-397b-a17b" />
    <label>timeout_secs</label><input id="m_timeout" value="60" />
    <label>max_tokens</label><input id="m_tokens" value="2048" />
    <label>temperature</label><input id="m_temp" value="0.2" />
    <button id="save_model">保存模型并继续</button>
    <h2>步骤3: 配置飞书通道</h2>
    <label>enabled</label><input id="f_enabled" value="false" />
    <label>app_id</label><input id="f_app_id" value="" />
    <label>app_secret</label><input id="f_app_secret" value="" />
    <label>verification_token</label><input id="f_token" value="" />
    <label>webhook_url</label><input id="f_webhook" value="" />
    <button id="save_feishu">保存飞书配置</button>
    <h2>步骤4: 智能体提示词（可选）</h2>
    <label>planner system</label><textarea id="p_sys">你是一个任务规划专家。</textarea>
    <label>planner user</label><textarea id="p_user">任务：{task_payload}</textarea>
    <label>executor system</label><textarea id="e_sys">你是一个执行专家。</textarea>
    <label>executor user</label><textarea id="e_user">步骤：{step_description}</textarea>
    <label>verifier system</label><textarea id="v_sys">你是一个验证专家。</textarea>
    <label>verifier user</label><textarea id="v_user">执行结果：{result}</textarea>
    <label>recovery system</label><textarea id="r_sys">你是一个恢复专家。</textarea>
    <label>recovery user</label><textarea id="r_user">错误：{error}，重试次数：{retry_count}</textarea>
    <button id="save_agents">保存提示词并完成</button>
  `;

  document.getElementById("save_model").onclick = async () => {
    await postJson("/api/config/models", {
      name: document.getElementById("m_name").value,
      protocol: document.getElementById("m_protocol").value,
      base_url: document.getElementById("m_base").value,
      api_key: document.getElementById("m_key").value,
      model: document.getElementById("m_model").value,
      timeout_secs: Number(document.getElementById("m_timeout").value),
      max_tokens: Number(document.getElementById("m_tokens").value),
      temperature: Number(document.getElementById("m_temp").value)
    });
    alert("模型已保存");
  };

  document.getElementById("save_feishu").onclick = async () => {
    await putJson("/api/config/channels/feishu", {
      feishu: {
        enabled: document.getElementById("f_enabled").value === "true",
        app_id: document.getElementById("f_app_id").value,
        app_secret: document.getElementById("f_app_secret").value,
        verification_token: document.getElementById("f_token").value,
        webhook_url: document.getElementById("f_webhook").value
      }
    });
    alert("飞书配置已保存");
  };

  document.getElementById("save_agents").onclick = async () => {
    await putJson("/api/config/agents", {
      planner: { system_prompt: document.getElementById("p_sys").value, user_prompt: document.getElementById("p_user").value },
      executor: { system_prompt: document.getElementById("e_sys").value, user_prompt: document.getElementById("e_user").value },
      verifier: { system_prompt: document.getElementById("v_sys").value, user_prompt: document.getElementById("v_user").value },
      recovery: { system_prompt: document.getElementById("r_sys").value, user_prompt: document.getElementById("r_user").value }
    });
    location.href = "/dashboard";
  };
}

async function renderDashboard() {
  wizard.classList.add("hidden");
  dashboard.classList.remove("hidden");
  const data = await getJson("/api/tasks");
  const tbody = document.querySelector("#tasks tbody");
  tbody.innerHTML = "";
  for (const t of data) {
    const tr = document.createElement("tr");
    tr.innerHTML = `<td>${t.id}</td><td>${t.title}</td><td>${t.status}</td><td>${t.step}</td><td>${t.retry_count}</td><td>${t.updated_at}</td>`;
    tbody.appendChild(tr);
  }
}

init().catch((e) => {
  wizard.innerHTML = `<pre>加载失败: ${String(e)}</pre>`;
});
