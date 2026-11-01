# Keyturn — AI Coding Accounts in Your Menu Bar

Manage accounts, workspaces and usage limits for AI coding assistants on macOS.

[English](#english-documentation) · [فارسی](#راهنمای-فارسی) · [Download](https://github.com/amirmahdinezhad/keyturn/releases/latest)

---

## English documentation

Keyturn keeps your coding accounts and usage limits in a menu-bar panel. Organize
Codex and Claude accounts into workspaces, see how much usage remains, and switch
saved Codex logins when you need another account.

### Features

- **Accounts** — official Codex and Claude browser sign-in, supported credential imports, account names and plan information.
- **Workspaces** — group accounts with separate accents, usage preferences and menu-bar settings.
- **Usage** — percentage bars, reset countdowns and optional menu-bar percentages or progress bars for a selected account.
- **Codex switching** — switch saved accounts manually or at a configured limit; confirm before closing a busy desktop session.
- **Appearance** — light, dark or system theme, workspace colors and optional warning colors near limits.
- **Updates** — check for a new release, review its changes, download it, then choose when to install and relaunch.

Codex and Claude have saved-account workflows. Installed, signed-in OpenCode and
Gemini CLIs can also be detected; OpenCode usage depends on its connected service,
and Gemini has no usage popup. Antigravity account support is experimental.

### Download & install

Requirements: **macOS 12 or later**, on Apple Silicon or Intel. The packaged app
does not require Node.js, Rust or a GitHub account. Provider CLIs and a supported
signed-in subscription are required for their account and usage features.

| Mac | Installer |
| --- | --- |
| Apple Silicon (M1 and newer) | [Download Keyturn DMG](https://github.com/amirmahdinezhad/keyturn/releases/latest/download/Keyturn_1.1.0_universal.dmg) |
| Intel | The same universal DMG |

1. Download and open the DMG.
2. Drag **Keyturn** into **Applications**, then launch it.
3. Click the Keyturn menu-bar icon to open the panel, create a workspace and add accounts.

This release uses ad-hoc signing and is **not notarized by Apple**. If macOS blocks
the first launch, open **System Settings → Privacy & Security → Open Anyway**
after attempting to launch. [All releases and changes](https://github.com/amirmahdinezhad/keyturn/releases).

### Update or remove

Open **Settings → Updates → Check for updates**. Expand **What's new** to read the
release notes, choose **Download**, then **Install & relaunch** when ready.
Optional automatic updates check once a day and download in the background;
installation still waits for your action. Manual checks can reveal a skipped version.

Pre-1.0 development builds using the old updater address need one manual install
of 1.0.0. Future releases then use the GitHub update flow.

To remove Keyturn, quit it from the panel and move Keyturn.app from Applications
to Trash. Saved local account data is retained.

### Local data

Keyturn keeps account credentials on your Mac. It contacts providers to read usage
and complete sign-in; update checks contact GitHub. Imported credentials are
stored locally with owner-only permissions and are never included in release
assets. [Storage details and development](docs/DEVELOPMENT.md#local-data).

---

## راهنمای فارسی

Keyturn حساب‌ها و سقف مصرف ابزارهای کدنویسی هوش مصنوعی را در نوار منوی مک
نمایش می‌دهد. حساب‌های Codex و Claude را در فضای کاری دسته‌بندی کنید، میزان
مصرف باقی‌مانده را ببینید و در صورت نیاز بین حساب‌های ذخیره‌شدهٔ Codex جابه‌جا شوید.

### قابلیت‌ها

- **حساب‌ها** — ورود رسمی Codex و Claude از مرورگر، واردکردن فایل ورود پشتیبانی‌شده، نام حساب و اطلاعات اشتراک.
- **فضای کاری** — گروه‌بندی حساب‌ها با رنگ و تنظیمات مستقل مصرف و نوار منو.
- **نمایش مصرف** — درصد، نوار پیشرفت و زمان بازنشانی در پنل؛ نمایش اختیاری مصرف حساب انتخاب‌شده در نوار منو.
- **جابه‌جایی Codex** — تغییر حساب دستی یا بر اساس حد مصرف، با درخواست تأیید پیش از بستن نشست فعال دسکتاپ.
- **ظاهر** — تم روشن، تیره یا هماهنگ با سیستم و رنگ هشدار نزدیک سقف مصرف.
- **آپدیت** — مشاهدهٔ نسخهٔ جدید و تغییرات، دانلود و سپس نصب با انتخاب شما.

OpenCode و Gemini نصب‌شده و واردشده نیز شناسایی می‌شوند؛ نمایش مصرف OpenCode
به سرویس متصل بستگی دارد و Gemini پنجرهٔ مصرف ندارد. پشتیبانی Antigravity آزمایشی است.

### دانلود و نصب

پیش‌نیاز: **macOS 12 یا جدیدتر** روی مک Apple Silicon یا Intel.
برای نصب بستهٔ آماده به Node.js، Rust یا حساب گیت‌هاب نیازی ندارید.
قابلیت‌های هر سرویس به CLI و اشتراک واردشدهٔ پشتیبانی‌شدهٔ همان سرویس نیاز دارند.

[دانلود Keyturn برای هر دو نوع مک](https://github.com/amirmahdinezhad/keyturn/releases/latest/download/Keyturn_1.1.0_universal.dmg)

1. فایل DMG را دانلود و باز کنید.
2. **Keyturn** را به **Applications** بکشید و اجرا کنید.
3. روی آیکون Keyturn در نوار منو کلیک کنید، فضای کاری بسازید و حساب‌ها را اضافه کنید.

این نسخه امضای محلی دارد و **توسط Apple notarize نشده است**. اگر macOS اولین
اجرا را مسدود کرد، پس از تلاش اجرا، از **System Settings → Privacy & Security → Open Anyway**
اجازهٔ اجرا بدهید. [همهٔ نسخه‌ها و تغییرات](https://github.com/amirmahdinezhad/keyturn/releases).

### به‌روزرسانی و حذف

در **Settings → Updates** روی **Check for updates** بزنید. تغییرات را در
**What's new** ببینید، **Download** را انتخاب کنید و هر وقت آماده بودید
**Install & relaunch** را بزنید. آپدیت خودکار اختیاری، روزی یک بار بررسی و در
پس‌زمینه دانلود می‌کند؛ نصب همچنان با انتخاب شما انجام می‌شود.

اگر نسخهٔ آزمایشی قبل از 1.0.0 با آدرس قدیمی آپدیت را دارید، نسخهٔ 1.0.0 را یک
بار دستی نصب کنید. نسخه‌های بعدی از داخل برنامه به‌روز می‌شوند.

برای حذف، ابتدا از پنل برنامه خارج شوید و Keyturn.app را از Applications به
Trash منتقل کنید. داده‌های محلی حساب‌ها باقی می‌مانند.

### داده‌های محلی

فایل‌های ورود روی مک شما نگهداری می‌شوند. برنامه برای ورود و خواندن مصرف به
ارائه‌دهنده و برای بررسی آپدیت به گیت‌هاب وصل می‌شود. اطلاعات ورود بخشی از
بستهٔ انتشار نیستند. [جزئیات ذخیره‌سازی](docs/DEVELOPMENT.md#local-data).

---

## Development

Built with Tauri v2, Rust, React, TypeScript and Tailwind CSS.

```bash
pnpm install --frozen-lockfile
pnpm run run
```

Development runs on **http://localhost:5174/**. The actual app is the native tray
panel; a browser preview has no Tauri IPC. See [development notes](docs/DEVELOPMENT.md),
[tests](docs/TESTING.md), [workspaces](docs/WORKSPACES.md),
[architecture](CONTRACT.md) and the [owner-approved release workflow](docs/RELEASING.md).
