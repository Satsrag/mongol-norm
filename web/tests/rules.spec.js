import { test, expect } from "@playwright/test";

for (const kind of ["zwnj", "zwj"]) {
  test(`${kind} reference translates text, tooltips and navigation without changing glyphs`, async ({ page }) => {
    const errors = [];
    page.on("pageerror", error => errors.push(error.message));
    await page.goto(`/${kind}-drop-rules/`);
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    await expect(page.locator("h1")).toHaveText(`${kind.toUpperCase()} drop rules`);
    const snapshot = () => page.locator(".render-queue").evaluateAll(queues => queues.map(queue => ({
      glyphs: [...queue.querySelectorAll("path")].map(path => path.getAttribute("d")),
      norm: [...queue.querySelectorAll(".norm-text")].map(node => node.textContent),
    })));
    const before = await snapshot();
    expect(before.length).toBe(4);
    expect(before.every(queue => queue.glyphs.length > 0)).toBe(true);
    await page.locator(".term").filter({ hasText: /^isol$/ }).first().focus();
    await expect(page.locator("#term-tooltip")).toHaveText("Isolated form.");
    await page.getByRole("button", { name: "中文", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await expect(page.locator("h1")).toHaveText(`${kind.toUpperCase()} drop 规则`);
    await expect(page.locator("nav").first()).toHaveAttribute("aria-label", `${kind.toUpperCase()} 规则目录`);
    await page.locator(".term").filter({ hasText: /^isol$/ }).first().focus();
    await expect(page.locator("#term-tooltip")).toHaveText("独立形。");
    expect(await snapshot()).toEqual(before);
    await page.reload();
    await expect(page.locator("html")).toHaveAttribute("lang", "zh-CN");
    await page.getByRole("button", { name: "English", exact: true }).click();
    await expect(page.locator("html")).toHaveAttribute("lang", "en");
    expect(await page.locator("i18n-text").evaluateAll(nodes => nodes.every(node => !/[\u4e00-\u9fff]/.test(node.textContent)))).toBe(true);
    await page.setViewportSize({ width: 390, height: 844 });
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
    await page.evaluate(() => document.fonts.ready);
    expect(await page.evaluate(() => document.fonts.check('27px "HudumReview"', "ᠮ"))).toBe(true);
    const urls = await page.locator("a[href]").evaluateAll(links => [...new Set(links.map(link => link.href).filter(url => new URL(url).origin === location.origin))]);
    for (const url of urls) {
      const response = await page.request.get(url);
      expect(response.ok(), url).toBe(true);
      const hash = new URL(url).hash;
      if (hash && new URL(url).pathname === new URL(page.url()).pathname) {
        expect(await page.locator(`[id="${decodeURIComponent(hash.slice(1))}"]`).count(), hash).toBe(1);
      }
    }
    expect(errors).toEqual([]);
  });
}
