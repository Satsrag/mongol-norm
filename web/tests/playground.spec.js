import { test, expect } from "@playwright/test";
import { execFileSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
// Use the native CLI as an independent reference for the browser binding.
execFileSync("cargo", ["build", "-p", "mongol-norm", "--bin", "mongol-norm", "--locked"], { cwd: root });
function cli(command, text) {
  return execFileSync("cargo", ["run", "--quiet", "--locked", "-p", "mongol-norm", "--bin", "mongol-norm", "--", command, text], { cwd: root, encoding: "utf8" }).replace(/\n$/, "");
}

async function ready(page) {
  await page.goto("/");
  await expect(page.locator("#input")).toBeEnabled({ timeout: 15000 });
  await expect(page.locator("#engine-status")).toContainText("本地计算");
}

test("loads the bundled font and matches the CLI for real words and controls", async ({ page }) => {
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await ready(page);
  expect(await page.evaluate(() => document.fonts.check('32px "Hudum"', "ᠮᠣᠩᠭᠣᠯ"))).toBe(true);
  await expect(page.locator("#engine-status")).not.toContainText("失败");
  for (const word of ["ᠮᠣᠩᠭᠣᠯ", "ᠰᠡᠢᠨ", "ᠪᠠ\u180b", "ᠯᠠᠯᠠ\u200dᠫᠦᠳ", "ᠠ\u180eᠠ", "\u202fᠤᠨ", "\u200dᠠ"]) {
    await page.locator("#input").fill(word);
    await expect(page.locator("#shape-output")).toHaveText(cli("shape", word));
    await expect(page.locator("#norm-output")).toHaveText(cli("normalize", word));
    await expect(page.locator("#shape-match")).toHaveText("✓ Shapes match / shape 序列一致");
    await expect(page.locator("#original-preview")).toHaveText(word);
    await expect(page.locator("#normalized-preview")).toHaveText(cli("normalize", word));
  }
  expect(errors).toEqual([]);
});

test("invalid input clears stale results and keeps the engine error", async ({ page }) => {
  await ready(page);
  for (const text of ["ᠠ\u200cᠠ", "hello", "ᠠ ᠠ", "ᠠ\nᠠ"]) {
    await page.locator("#input").fill(text);
    await expect(page.locator("#shape-error")).toBeVisible();
    await expect(page.locator("#norm-error")).toBeVisible();
    await expect(page.locator("#shape-error")).toContainText("non-Mongolian character");
    await expect(page.locator("#norm-output")).toBeEmpty();
    await expect(page.locator("#normalized-preview")).toBeEmpty();
    await expect(page.locator("#copy-norm")).toBeDisabled();
    await expect(page.locator("#shape-match")).toHaveText("Cannot compare / 无法比较");
  }
  await page.locator("[data-sample=sain]").click();
  await expect(page.locator("#shape-error")).toBeHidden();
  await expect(page.locator("#shape-output")).toHaveText("S+A+I+I+A");
});

test("inserts invisible controls at the caret and replaces a selection", async ({ page }) => {
  await ready(page);
  await page.locator("#input").fill("ᠪᠠ");
  await page.locator("#input").evaluate((node) => node.setSelectionRange(2, 2));
  await page.locator("[data-insert='180B']").click();
  await expect(page.locator("#input")).toHaveValue("ᠪᠠ\u180b");
  await expect(page.locator("#input-codepoints")).toContainText("U+180B FVS1");
  await page.locator("#input").evaluate((node) => node.setSelectionRange(2, 3));
  await page.locator("[data-insert='200D']").click();
  await expect(page.locator("#input")).toHaveValue("ᠪᠠ\u200d");
  await expect(page.locator("#input-codepoints")).toContainText("U+200D ZWJ");
});

test("copy preserves the exact normalized text and shape separators", async ({ page, context }) => {
  await context.grantPermissions(["clipboard-read", "clipboard-write"]);
  await ready(page);
  await page.locator("#copy-norm").click();
  await expect(page.locator("#copy-norm")).toHaveText("Copied / 已复制");
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(cli("normalize", "ᠮᠣᠩᠭᠣᠯ"));
  await page.locator("#copy-shape").click();
  await expect(page.locator("#copy-shape")).toHaveText("Copied / 已复制");
  expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(cli("shape", "ᠮᠣᠩᠭᠣᠯ"));
});

test("clearing and rapid edits never retain an earlier result", async ({ page }) => {
  await ready(page);
  await page.locator("#input").fill("hello");
  await page.locator("#clear").click();
  await expect(page.locator("#input")).toHaveValue("");
  await expect(page.locator("#shape-output")).toContainText("输入蒙古文后");
  await expect(page.locator("#input-codepoints")).toBeEmpty();
  await expect(page.locator("#copy-shape")).toBeDisabled();
  await page.locator("#input").fill("ᠠ");
  await page.locator("#input").fill("ᠰᠡᠢᠨ");
  await expect(page.locator("#shape-output")).toHaveText("S+A+I+I+A");
  await expect(page.locator("#shape-error")).toBeHidden();
});

test("waits for input-method composition to finish", async ({ page }) => {
  await ready(page);
  await page.locator("#input").evaluate((node) => {
    node.dispatchEvent(new CompositionEvent("compositionstart"));
    node.value = "ᠰᠡᠢᠨ";
    node.dispatchEvent(new InputEvent("input", { isComposing: true }));
  });
  await expect(page.locator("#shape-output")).toHaveText("Computing… / 计算中…");
  await page.locator("#input").dispatchEvent("compositionend");
  await expect(page.locator("#shape-output")).toHaveText("S+A+I+I+A");
});

test("mobile layout keeps all content within the viewport", async ({ page }) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await ready(page);
  await expect(page.locator("#shape-output")).not.toBeEmpty();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.locator("#input").fill("ᠠ".repeat(120));
  await expect(page.locator("#copy-norm")).toBeEnabled();
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
});

test("a failed WASM load shows an actionable state", async ({ page }) => {
  await page.route("**/pkg/*.wasm", (route) => route.abort());
  await page.goto("/");
  await expect(page.locator("#engine-status")).toHaveText("Engine failed to load / 引擎加载失败");
  await expect(page.locator("#shape-output")).toContainText("请刷新页面重试");
  await expect(page.locator("#input")).toBeDisabled();
  await expect(page.locator("#copy-shape")).toBeDisabled();
});
