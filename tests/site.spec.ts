import { test, expect, type Page } from "@playwright/test";
import { pageNames, pages, route } from "../lib/pages";

const address = "1BoatSLRHtKNngkdXEeobR76b53LETtpyT";
const cashAddress = "bitcoincash:qpm2qsznhks23z7629mms6s4cwef74vcwvy22gdx6a";

async function mockSources(page: Page) {
  await page.route(
    "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json*",
    (request) =>
      request.fulfill({
        json: {
          schemaVersion: 1,
          source: "snapshot",
          updatedAt: new Date().toISOString(),
          node: {
            chain: "main",
            blocks: 975351,
            headers: 975351,
            verificationProgress: 1,
            initialBlockDownload: false,
          },
          pool: {
            ready: true,
            hashrateHps: 1500000000000,
            workers: 2,
            bestShare: 12345,
            blocksFound: 0,
          },
          history: [
            {
              time: new Date(Date.now() - 60000).toISOString(),
              hashrateHps: 1000000000000,
            },
            { time: new Date().toISOString(), hashrateHps: 1500000000000 },
          ],
          checks: { bitcoin: "ok", stratum: "ok" },
        },
      }),
  );
  await page.route("https://api.bdl-pool.com:36018/api/v1/**", (request) => {
    const path = new URL(request.request().url()).pathname;
    const updatedAt = new Date().toISOString();
    if (path.endsWith("/health"))
      return request.fulfill({ json: { apiVersion: "1", status: "ok" } });
    if (path.endsWith("/online"))
      return request.fulfill({
        json: {
          apiVersion: "1",
          available: true,
          stale: false,
          total: 0,
          miners: [],
          updatedAt,
        },
      });
    if (path.endsWith("/workers"))
      return request.fulfill({
        json: { apiVersion: "1", address, workers: [], total: 0, offset: 0 },
      });
    return request.fulfill({
      json: {
        apiVersion: "1",
        address,
        updatedAt,
        presenceAvailable: true,
        presenceUpdatedAt: updatedAt,
        connections: 1,
        shares: 42,
        bestShare: 12,
        hashrateHps: { "5m": 1200000000000 },
      },
    });
  });
}

test.beforeEach(async ({ page }) => {
  await mockSources(page);
});

for (const lang of ["fr", "en"] as const) {
  for (const name of pageNames) {
    test(`${lang}: ${name} renders with the same route, language and metadata`, async ({
      page,
    }) => {
      const errors: string[] = [];
      page.on("pageerror", (error) => errors.push(error.message));
      page.on("console", (message) => {
        if (message.type() === "error") errors.push(message.text());
      });
      const response = await page.goto(route(name, lang));
      expect(response?.status()).toBe(200);
      await expect(page.locator("html")).toHaveAttribute("lang", lang);
      await expect(page.locator("h1")).toBeVisible();
      await expect(page).toHaveTitle(
        name === "home"
          ? "BDL Pool"
          : `${pages[name].labels[lang === "en" ? 1 : 0]} · BDL Pool`,
      );
      await expect(page.locator('link[rel="canonical"]')).toHaveAttribute(
        "href",
        `https://bdl-pool.com${route(name, lang)}`,
      );
      await expect(
        page.locator('#main-nav a[aria-current="page"]'),
      ).toHaveAttribute("href", route(name, lang));
      await expect(page.locator(".language-switch")).toHaveAttribute(
        "href",
        new RegExp(route(name, lang === "fr" ? "en" : "fr")),
      );
      await expect(page.locator("body")).toHaveAttribute("data-page", name);
      if (name === "home" || name === "statistics")
        await expect(page.locator("#pool-badge")).toHaveText(
          lang === "en" ? "Pool available" : "Pool disponible",
        );
      if (name === "miners")
        await expect(page.locator("#online-message")).toHaveText(
          lang === "en" ? "No miners connected." : "Aucun mineur connecté.",
        );
      if (name === "api") {
        await page.locator("#api-health-test").click();
        await expect(page.locator("#api-health-result")).toContainText(
          '"status": "ok"',
        );
      }
      expect(errors).toEqual([]);
    });
  }
}

test("connection form preserves V1/V2 settings, validation and language links", async ({
  page,
}) => {
  await page.goto("/connexion/?protocol=v2");
  await expect(page.locator("#endpoint-port")).toHaveText("36809");
  await expect(page.locator(".language-switch")).toHaveAttribute(
    "href",
    /\/en\/connect\/\?protocol=v2$/,
  );
  await page.locator("#btc-address").fill("invalid");
  await page.locator('#miner-form button[type="submit"]').click();
  await expect(page.locator("#form-error")).toBeVisible();
  await page.locator("#btc-address").fill(address);
  await page.locator("#worker-name").fill("bitaxe-01");
  await page.locator('#miner-form button[type="submit"]').click();
  await expect(page.locator("#result-user")).toHaveText(address + ".bitaxe-01");
  await expect(page.locator("#result-url")).toContainText(
    "stratum2+tcp://stratum.bdl-pool.com:36809/",
  );
  await expect(page.locator("#result-key")).not.toBeEmpty();
  await page.locator('[data-protocol="v1"]').click();
  await expect(page.locator("#endpoint-port")).toHaveText("39991");
  await expect(page.locator("#miner-result")).toBeHidden();
  await page.locator('#miner-form button[type="submit"]').click();
  await expect(page.locator("#result-url")).toHaveText(
    "stratum+tcp://stratum.bdl-pool.com:39991",
  );
  await expect(page.locator("#result-key-row")).toBeHidden();
});

test("theme persists across navigation and reload", async ({ page }) => {
  await page.goto("/");
  await expect(page.locator("body")).toHaveAttribute("data-page", "home");
  await page.locator("#theme-toggle").click();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await page.locator('#main-nav a[href="/faq/"]').click();
  await expect(page).toHaveURL(/\/faq\/$/);
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
  await expect(page.locator("#theme-toggle")).toHaveAttribute(
    "aria-pressed",
    "false",
  );
  await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute("content", "#f6f7fc");
  await page.reload();
  await expect(page.locator("html")).toHaveAttribute("data-theme", "light");
});

test("mobile menu, keyboard dismissal and layout remain usable", async ({
  page,
}) => {
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto("/");
  await expect(page.locator("body")).toHaveAttribute("data-page", "home");
  const toggle = page.locator(".menu-toggle");
  await toggle.click();
  await expect(toggle).toHaveAttribute("aria-expanded", "true");
  await expect(page.locator("#main-nav")).toBeVisible();
  await page.keyboard.press("Escape");
  await expect(toggle).toHaveAttribute("aria-expanded", "false");
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
  ).toBe(true);
  await toggle.click();
  await page.locator('#main-nav a[href="/connexion/"]').click();
  await expect(page).toHaveURL(/\/connexion\/$/);
  await expect(page.locator("#endpoint-port")).toHaveText("39991");
});

test("statistics loads Chart.js after its dependencies and refreshes once per click", async ({
  page,
}) => {
  let reads = 0;
  page.on("request", (request) => {
    if (request.url().includes("/stats/status.json")) reads++;
  });
  await page.goto("/statistiques/");
  await expect(page.locator("#metric-hashrate")).toContainText("TH/s");
  await expect(page.locator("#cryptoChart")).toBeVisible();
  expect(reads).toBe(1);
  await page.locator("#refresh-status").click();
  await expect(page.locator("#refresh-status")).not.toBeDisabled();
  expect(reads).toBe(2);
});

test("miner lookup validates the address and shows returned API data", async ({
  page,
}) => {
  await page.goto("/mineurs/");
  await expect(page.locator("#online-message")).toHaveText(
    "Aucun mineur connecté.",
  );
  await page.locator("#miner-lookup-address").fill("invalid");
  await page.locator("#miner-search button").click();
  await expect(page.locator("#miner-search-message")).toHaveText(
    "Vérifiez votre adresse Bitcoin.",
  );
  await page.locator("#miner-lookup-address").fill(address);
  await page.locator("#miner-search button").click();
  await expect(page.locator("#personal-shares")).toHaveText("42");
  await expect(page.locator("#personal-hashrate")).toContainText("TH/s");
});

test("unavailable monitoring keeps unknown statistics empty", async ({
  page,
}) => {
  await page.route(
    "https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status.json*",
    (request) => request.abort(),
  );
  await page.goto("/statistiques/");
  await expect(page.locator("#pool-badge")).toHaveText("Suivi indisponible");
  for (const id of ["metric-hashrate", "metric-workers", "metric-blocks"])
    await expect(page.locator("#" + id)).toHaveText("—");
});

test("old hash links redirect to the corresponding category", async ({
  page,
}) => {
  await page.goto("/en/#connexion");
  await expect(page).toHaveURL(/\/en\/connect\/$/);
  await expect(page.locator("#endpoint-port")).toHaveText("39991");
});

test("export includes public contracts and crawl files, excludes server code", async ({
  request,
}) => {
  expect((await request.get("/api/openapi.json")).status()).toBe(200);
  expect(await (await request.get("/CNAME")).text()).toContain("bdl-pool.com");
  expect(await (await request.get("/sitemap.xml")).text()).toContain(
    "https://bdl-pool.com/en/statistics/",
  );
  expect((await request.get("/robots.txt")).status()).toBe(200);
  expect((await request.get("/api/server.py")).status()).toBe(404);
  expect((await request.get("/monitoring/collect.py")).status()).toBe(404);
});

test("English content is readable without JavaScript", async ({ browser }) => {
  const context = await browser.newContext({ javaScriptEnabled: false });
  const page = await context.newPage();
  await page.goto("http://127.0.0.1:3000/en/");
  await expect(page.locator("html")).toHaveAttribute("lang", "en");
  await expect(page.locator("h1")).toContainText("BDL Pool");
  await expect(page.locator("noscript .noscript-message")).toContainText("Enable JavaScript");
  await context.close();
});

async function mockBCH(page: Page) {
  await page.route("https://api.bdl-pool.com:36018/api/v1/coins/bch/**", request => {
    const path = new URL(request.request().url()).pathname;
    const updatedAt = new Date().toISOString();
    const common = {apiVersion: "1", coin: "bch"};
    if (path.endsWith("/stratum")) return request.fulfill({json: {...common, v1: {host: "bch.example.com", port: 39992}, v2: {configured: false}}});
    if (path.endsWith("/online")) return request.fulfill({json: {...common, available: true, stale: false, updatedAt, total: 0, miners: []}});
    if (path.endsWith("/workers")) return request.fulfill({json: {...common, address: cashAddress, workers: [], total: 0, offset: 0}});
    return request.fulfill({json: {...common, address: cashAddress, updatedAt, shares: 987, hashrateHps: {"5m": 2000000}}});
  });
  await page.route("https://raw.githubusercontent.com/bdlglua/bdl-pool/stats/status-bch.json*", request => request.fulfill({json: {
    schemaVersion: 1, coin: "bch", source: "snapshot", updatedAt: new Date().toISOString(),
    node: {chain: "main", blocks: 900123, headers: 900123, verificationProgress: 1, initialBlockDownload: false},
    pool: {ready: true, hashrateHps: 2000000, workers: 1}, checks: {bitcoin: "ok", stratum: "job_received"}, history: []
  }}));
}

for (const lang of ["fr", "en"] as const) {
  test(`${lang}: BCH settings validate CashAddr and preserve the network across pages`, async ({page}) => {
    await mockBCH(page);
    await page.goto(route("connect", lang)+"?coin=bch&protocol=v2");
    await expect(page.locator("#pool-coin")).toHaveValue("bch");
    await expect(page.locator("#endpoint-host")).toHaveText("bch.example.com");
    await expect(page.locator("#endpoint-port")).toHaveText("39992");
    await expect(page.locator('[data-protocol="v2"]')).toBeHidden();
    await expect(page.locator('.language-switch')).toHaveAttribute("href", /coin=bch/);
    await expect(page.locator('#node-blocks')).toHaveCount(0);
    await page.locator("#btc-address").fill(address);
    await page.locator('#miner-form button[type="submit"]').click();
    await expect(page.locator("#form-error")).toContainText("CashAddr");
    await page.locator("#btc-address").fill(cashAddress.slice(0,-1)+"q");
    await page.locator('#miner-form button[type="submit"]').click();
    await expect(page.locator("#miner-result")).toBeHidden();
    await page.locator("#btc-address").fill(cashAddress.toUpperCase());
    await page.locator("#worker-name").fill("bitaxe-bch");
    await page.locator('#miner-form button[type="submit"]').click();
    await expect(page.locator("#result-user")).toHaveText(cashAddress+".bitaxe-bch");
    await expect(page.locator("#result-url")).toHaveText("stratum+tcp://bch.example.com:39992");
    await expect(page.locator("#result-key-row")).toBeHidden();
    const statsLink = page.locator(`#main-nav a[href*="${route("statistics",lang)}"]`);
    await statsLink.focus();
    await statsLink.click();
    await expect(page).toHaveURL(/coin=bch/);
    await expect(page.locator("#node-blocks")).toContainText(/900.123/);
    await expect(page.locator("#metric-hashrate")).toContainText("MH/s");
    await page.locator("#pool-coin").selectOption("btc");
    await expect(page.locator("#pool-coin")).toHaveValue("btc");
    await expect(page.locator("#metric-hashrate")).toContainText("TH/s");
  });
}

test("BCH miner lookup uses only BCH endpoints and canonical CashAddr", async ({page}) => {
  await mockBCH(page);
  const requests: string[] = [];
  page.on("request", request => { if (request.url().includes("/api/v1/")) requests.push(request.url()); });
  await page.goto("/mineurs/?coin=bch");
  await page.locator("#miner-lookup-address").fill(cashAddress.split(":")[1]);
  await page.locator("#miner-search button").click();
  await expect(page.locator("#personal-shares")).toHaveText("987");
  expect(requests.every(url => url.includes("/api/v1/coins/bch/"))).toBe(true);
});

test("the mobile selector opens BCH and its API documentation uses the BCH base URL", async ({page}) => {
  await mockBCH(page);
  await page.setViewportSize({width: 390, height: 844});
  await page.goto("/");
  await page.locator("#pool-coin").selectOption("bch");
  await expect(page).toHaveURL(/coin=bch/);
  await expect(page.locator("h1")).toContainText("Bitcoin Cash");
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.goto("/api/?coin=bch");
  await expect(page.locator("#api-base-url")).toHaveText("https://api.bdl-pool.com:36018/api/v1/coins/bch");
});

test("an unconfigured BCH server never displays BTC endpoints or cached BTC readings", async ({page}) => {
  await page.goto("/statistiques/");
  await expect(page.locator("#metric-hashrate")).toContainText("TH/s");
  await page.route("**/stats/status-bch.json*", request => request.abort());
  await page.route("**/api/v1/coins/bch/**", request => request.fulfill({status: 503, json: {error: {code: "coin_not_configured"}}}));
  await page.goto("/statistiques/?coin=bch");
  await expect(page.locator("#metric-hashrate")).toHaveText("—");
  await expect(page.locator("#node-blocks")).toHaveText("—");
  await page.goto("/connexion/?coin=bch");
  await expect(page.locator("#endpoint-host")).toHaveText("Non configuré");
  await expect(page.locator("#endpoint-port")).toHaveText("—");
  await expect(page.locator('#miner-form button[type="submit"]')).toBeDisabled();
});
