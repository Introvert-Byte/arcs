# Arcs — Backup Studio

**Backup apps and settings on this computer. Restore them later or on another machine.**

One program. No account required. Your files stay on your device.

---

## What it does

1. **Scan** — finds applications and config folders  
2. **Select** — choose what you want to keep  
3. **Backup** — creates an encrypted `.arcs` archive  
4. **Restore** — puts everything back when you need it  

Works on **Linux**, **Windows**, and **macOS**.

---

## Download & install

### Option A — Ready-made build

If you have a release package for your system, extract it and run:

- **Linux / macOS:** `./arcs`
- **Windows:** double-click `arcs.exe`

### Option B — Build from this folder

**Linux or macOS**

```bash
bash setup.sh
```

Then start the app:

```bash
./target/release/arcs
```

Or build and open in one step:

```bash
bash setup.sh --run
```

**Windows (PowerShell)**

```powershell
powershell -ExecutionPolicy Bypass -File .\setup.ps1 -Run
```

The first build can take a few minutes while tools are installed.

---

## How to use (simple)

| Step | What to do |
|------|------------|
| 1 | Open **Overview** → click **Scan** |
| 2 | Open **Applications** → tick what you need |
| 3 | Open **Backup** → set a password → **Create archive** |
| 4 | Later: **Restore** → pick the `.arcs` file → try **Dry-run** first |

**Tip:** Keep your password safe. If you encrypt a backup and lose the password, it cannot be opened.

---

## Themes & look

Open **Settings** to:

- Switch **Light**, **Dark**, or **Glass**
- Change colors, corner size, and transparency  
- Apply a **style addon** from the `addons` folder  

Your choices can be saved so they stay next time you open the app.

---

## Other ways to run

```bash
./target/release/arcs          # desktop window
./target/release/arcs --tui    # terminal screen
./target/release/arcs discover # list what was found (text)
```

---

## Privacy

- No sign-in is required (Guest mode is fine)  
- Optional local profile name only lives on **this** computer  
- Archive passwords are **never** stored by the app  
- No cloud account is needed for normal backups  

---

## Need help?

Open **Help** inside the app for a short guided tour.

Linux note: if the window does not start, install graphics libraries once:

```bash
sudo apt install libgtk-3-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libssl-dev
```

Then run `bash setup.sh` again.
