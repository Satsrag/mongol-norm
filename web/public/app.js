const $ = (id) => document.getElementById(id);
const input = $("input");
const controls = new Map([
  [0x180a, "NIRUGU"], [0x180b, "FVS1"], [0x180c, "FVS2"],
  [0x180d, "FVS3"], [0x180e, "MVS"], [0x180f, "FVS4"],
  [0x200c, "ZWNJ"], [0x200d, "ZWJ"], [0x202f, "NNBSP"],
  [0x20, "SPACE"], [0xa, "LF"], [0xd, "CR"], [0x9, "TAB"],
]);
const samples = {
  mongol: "ᠮᠣᠩᠭᠣᠯ",
  sain: "ᠰᠡᠢᠨ",
  zwj: "ᠯᠠᠯᠠ\u200dᠫᠦᠳ",
};
let engine;
let shapeResult = "";
let normResult = "";
let timer;
let composing = false;

function renderCodepoints(target, text) {
  const fragment = document.createDocumentFragment();
  for (const char of text) {
    const cp = char.codePointAt(0);
    const label = controls.get(cp);
    const tag = document.createElement("span");
    tag.className = "codepoint" + (label ? " control" : "") + (cp === 0x200c ? " unsupported" : "");
    const hex = `U+${cp.toString(16).toUpperCase().padStart(4, "0")}`;
    tag.textContent = label ? `${hex} ${label}` : hex;
    tag.title = label || char;
    fragment.append(tag);
  }
  target.replaceChildren(fragment);
}

function resetResults(message) {
  shapeResult = "";
  normResult = "";
  $("shape-output").replaceChildren();
  const placeholder = document.createElement("span");
  placeholder.className = "placeholder";
  placeholder.textContent = message;
  $("shape-output").append(placeholder);
  $("norm-output").textContent = "";
  $("normalized-preview").textContent = "";
  $("norm-codepoints").replaceChildren();
  $("norm-placeholder").textContent = message;
  $("norm-placeholder").hidden = false;
  $("shape-error").hidden = true;
  $("norm-error").hidden = true;
  $("norm-state").hidden = true;
  $("copy-shape").disabled = true;
  $("copy-norm").disabled = true;
  $("unit-count").textContent = "—";
  $("shape-match").textContent = "Waiting for input / 等待输入";
  $("shape-match").classList.remove("match");
}

function showError(id, error) {
  const node = $(id);
  const message = String(error);
  const explanation = message.startsWith("non-Mongolian character")
    ? "\n输入含不支持的字符；shape / norm 仅接受单个蒙古文词及支持的控制符。"
    : (/[^\x00-\x7F]/.test(message) ? "" : "\n计算失败，详情请查看上方错误信息。");
  node.textContent = message + explanation;
  node.hidden = false;
}

function refreshInput() {
  const text = input.value;
  $("input-count").textContent = `${Array.from(text).length} code points / 码点`;
  $("original-preview").textContent = text;
  renderCodepoints($("input-codepoints"), text);
  return text;
}

function calculate() {
  clearTimeout(timer);
  const text = refreshInput();
  resetResults(text ? "Computing… / 计算中…" : "Enter Mongolian to see results / 输入蒙古文后，结果会显示在这里");
  if (!engine || !text) return;
  if (text.length > 2048) {
    resetResults("Input too long: limit 2048 UTF-16 units / 输入过长，请限制在 2048 个 UTF-16 单元以内。");
    return;
  }

  let shaped = false;
  try {
    shapeResult = engine.shape(text);
    shaped = true;
    const units = shapeResult ? shapeResult.split("+") : [];
    const fragment = document.createDocumentFragment();
    units.forEach((unit, index) => {
      if (index) {
        const plus = document.createElement("span");
        plus.className = "shape-plus";
        plus.textContent = "+";
        fragment.append(plus);
      }
      const item = document.createElement("span");
      item.className = "shape-unit";
      item.textContent = unit;
      fragment.append(item);
    });
    $("shape-output").replaceChildren(fragment);
    $("unit-count").textContent = `${units.length} units / 单元`;
    $("copy-shape").disabled = !shapeResult;
  } catch (error) {
    $("shape-output").replaceChildren();
    showError("shape-error", error);
  }

  $("norm-placeholder").hidden = true;
  try {
    normResult = engine.normalize(text);
    $("norm-output").textContent = normResult;
    $("normalized-preview").textContent = normResult;
    renderCodepoints($("norm-codepoints"), normResult);
    $("copy-norm").disabled = !normResult;
    $("norm-state").textContent = text === normResult ? "Encoding unchanged / 编码未变" : "Encoding updated / 编码已更新";
    $("norm-state").hidden = false;
    const match = shaped && engine.shape(normResult) === shapeResult;
    $("shape-match").textContent = match ? "✓ Shapes match / shape 序列一致" : "Shapes differ / shape 序列不同";
    $("shape-match").classList.toggle("match", match);
    if (!normResult) {
      $("norm-placeholder").textContent = "Normalized result is empty / 规范化结果为空";
      $("norm-placeholder").hidden = false;
    }
  } catch (error) {
    showError("norm-error", error);
    $("shape-match").textContent = "Cannot compare / 无法比较";
  }
}

input.addEventListener("compositionstart", () => { composing = true; clearTimeout(timer); });
input.addEventListener("compositionend", () => { composing = false; calculate(); });
input.addEventListener("input", () => {
  clearTimeout(timer);
  refreshInput();
  resetResults("Computing… / 计算中…");
  if (!composing) timer = setTimeout(calculate, 100);
});
$("clear").addEventListener("click", () => { input.value = ""; calculate(); input.focus(); });
document.querySelectorAll("[data-sample]").forEach((button) => {
  button.addEventListener("click", () => {
    input.value = samples[button.dataset.sample];
    calculate();
    input.focus();
  });
});
document.querySelectorAll("[data-insert]").forEach((button) => {
  // Keep the insertion point when a pointer moves focus from the textarea.
  button.addEventListener("mousedown", (event) => event.preventDefault());
  button.addEventListener("click", () => {
    if (input.value.length - (input.selectionEnd - input.selectionStart) >= input.maxLength) return;
    input.setRangeText(String.fromCodePoint(parseInt(button.dataset.insert, 16)), input.selectionStart, input.selectionEnd, "end");
    calculate();
    input.focus();
  });
});

async function copy(button, text) {
  try {
    await navigator.clipboard.writeText(text);
    button.textContent = "Copied / 已复制";
    $("announcement").textContent = "Copied to clipboard / 已复制到剪贴板";
  } catch {
    button.textContent = "Copy failed / 复制失败";
    $("announcement").textContent = "Clipboard unavailable. Select the result and copy manually. / 无法访问剪贴板，请选中结果后手动复制。";
  }
  setTimeout(() => { button.textContent = "Copy / 复制"; }, 1600);
}
$("copy-shape").addEventListener("click", (event) => copy(event.currentTarget, shapeResult));
$("copy-norm").addEventListener("click", (event) => copy(event.currentTarget, normResult));

async function start() {
  const actions = document.querySelectorAll("[data-insert], [data-sample], #clear");
  actions.forEach((button) => { button.disabled = true; });
  refreshInput();
  const fontReady = document.fonts.load('32px "Hudum"', "ᠮᠣᠩᠭᠣᠯ").then((fonts) => fonts.length > 0, () => false);
  try {
    const { default: init, Engine } = await import("./pkg/mongol_norm.js");
    await init();
    engine = new Engine();
    input.disabled = false;
    actions.forEach((button) => { button.disabled = false; });
    calculate();
    const fontLoaded = await fontReady;
    $("engine-status").textContent = `Computed locally / 本地计算 · v${engine.version()} · ${engine.canonical_version()}${fontLoaded ? "" : " · Hudum font failed to load / Hudum 字体加载失败"}`;
  } catch (error) {
    resetResults("Engine failed to load. Refresh to retry. / 引擎加载失败，请刷新页面重试。");
    $("engine-status").textContent = "Engine failed to load / 引擎加载失败";
    showError("shape-error", `Unable to load the shaping engine / 无法加载字形引擎：${String(error)}`);
  }
}

start();
