import { expect, test, type Page } from "@playwright/test";

const PASSWORD = "a long enough passphrase";
const unique = () => `e2e-${Date.now()}-${Math.floor(Math.random() * 1e6)}@example.com`;

async function register(page: Page, email: string, name = "Ada") {
  await page.goto("/register");
  await page.getByLabel("Your name").fill(name);
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password", { exact: true }).fill(PASSWORD);
  await page.getByLabel("Repeat password").fill(PASSWORD);
  await page.getByRole("button", { name: "Create account" }).click();
  await expect(page.getByRole("heading", { name: `Hello, ${name}` })).toBeVisible();
}

test("register, create and edit a project, sign out, sign back in", async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  const email = unique();
  await register(page, email);

  // The session lives in HttpOnly cookies: page scripts can read only the CSRF cookie.
  const visible = await page.evaluate(() => document.cookie);
  expect(visible).toContain("forge_csrf=");
  expect(visible).not.toContain("forge_at=");
  expect(visible).not.toContain("forge_rt=");

  await page.getByRole("link", { name: "Create a project" }).click();
  await page.getByLabel("Name").fill("Weather app");
  await page.getByLabel(/^Analysis/).check();
  await page.getByRole("button", { name: "Create project" }).click();
  await expect(page.getByRole("heading", { name: "Weather app" })).toBeVisible();
  await expect(page.getByText("Clean room (outside only)")).toBeVisible();

  await page.getByRole("button", { name: "Rename" }).click();
  await page.getByLabel("Name").fill("Weather study");
  await page.getByRole("button", { name: "Save" }).click();
  await expect(page.getByRole("heading", { name: "Weather study" })).toBeVisible();

  await page.getByRole("complementary").getByRole("link", { name: "Projects" }).click();
  await expect(page.getByText("Weather study")).toBeVisible();

  await page.getByRole("complementary").getByRole("link", { name: "Security" }).click();
  await expect(page.getByText("this device")).toBeVisible();

  await page.getByRole("complementary").getByRole("button", { name: "Sign out" }).click();
  await expect(page.getByRole("heading", { name: "Sign in" })).toBeVisible();

  // Signed out, a protected page sends you to sign-in and back.
  await page.goto("/projects");
  await expect(page).toHaveURL(/\/login\?next=%2Fprojects/);
  await page.getByLabel("Email").fill(email);
  await page.getByLabel("Password").fill(PASSWORD);
  await page.getByRole("button", { name: "Sign in" }).click();
  await expect(page).toHaveURL(/\/projects$/);
  await expect(page.getByText("Weather study")).toBeVisible();
});

test("an expired access cookie is renewed with the refresh cookie", async ({ page, context }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await register(page, unique());
  // Simulate the 10-minute access token running out.
  const kept = (await context.cookies()).filter((c) => c.name !== "forge_at");
  await context.clearCookies();
  await context.addCookies(kept);
  await page.goto("/settings/security");
  await expect(page.getByText("this device")).toBeVisible();
  expect((await context.cookies()).some((c) => c.name === "forge_at")).toBe(true);
});

test("a wrong password shows the error and a request id", async ({ page }) => {
  await page.goto("/login");
  await page.getByLabel("Email").fill(unique());
  await page.getByLabel("Password").fill("not the right one");
  await page.getByRole("button", { name: "Sign in" }).click();
  const alert = page.getByRole("alert");
  await expect(alert).toContainText("The email or password is wrong.");
  await expect(alert).toContainText(/Request ID: [0-9a-f-]{36}/);
});

for (const [name, width, expectations] of [
  ["mobile", 375, { bottom: true, sidebar: false, wideName: false }],
  ["tablet", 900, { bottom: false, sidebar: true, wideName: false }],
  ["desktop", 1440, { bottom: false, sidebar: true, wideName: true }],
] as const) {
  test(`layout at ${width}px (${name})`, async ({ page }) => {
    await page.setViewportSize({ width, height: 900 });
    await register(page, unique());
    await page.goto("/projects");
    await expect(page.getByRole("heading", { name: "Projects" })).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Bottom navigation" })).toBeVisible({ visible: expectations.bottom });
    await expect(page.getByRole("complementary", { name: "Main navigation" })).toBeVisible({ visible: expectations.sidebar });
    if (expectations.sidebar) {
      await expect(page.getByRole("complementary").getByText("Tophunt Forge")).toBeVisible({ visible: expectations.wideName });
    }
    // The content starts right after the sidebar (or at the left edge): no empty column between.
    const mainLeft = await page.getByRole("main").evaluate((m) => m.getBoundingClientRect().left);
    const sideRight = expectations.sidebar ? await page.getByRole("complementary").evaluate((a) => a.getBoundingClientRect().right) : 0;
    expect(Math.abs(mainLeft - sideRight)).toBeLessThanOrEqual(1);
    // Dark theme by default.
    const bg = await page.evaluate(() => getComputedStyle(document.body).backgroundColor);
    expect(bg).toBe("rgb(11, 16, 32)");
    // Nothing wider than the screen.
    const overflow = await page.evaluate(() => document.documentElement.scrollWidth - window.innerWidth);
    expect(overflow).toBeLessThanOrEqual(0);
    await page.screenshot({ path: `test-results/layout-${name}.png`, fullPage: true });
  });
}

test("a refresh cut off by navigation doesn't sign the person out", async ({ page, context }) => {
  await page.setViewportSize({ width: 1440, height: 900 });
  await register(page, unique());
  await page.waitForLoadState("networkidle");
  const kept = (await context.cookies()).filter((c) => c.name !== "forge_at");
  await context.clearCookies();
  await context.addCookies(kept);
  // Leave the dashboard while its refresh is in flight, so the new cookie it sets may be lost.
  await page.goto("/");
  await page.goto("/settings/security");
  await expect(page.getByText("this device")).toBeVisible();
});

test("help and diagnostics works signed out", async ({ page }) => {
  await page.goto("/help");
  await expect(page.getByText("The server answers.")).toBeVisible();
  await expect(page.getByText("The database is reachable and up to date.")).toBeVisible();
  await expect(page.getByText(/Tophunt Forge \d+\.\d+\.\d+, API v1/)).toBeVisible();
});
